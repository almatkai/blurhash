/* Source TSX is compiled unchanged in a VM. Only enumerated fake imports exist.
 * No host fs/os/child_process/API/fetch/process/require is exposed to source. */
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const ts = require('../.work/v1/node_modules/typescript');
const library = require('../fixtures/library/blurhash-2.0.5.cjs');
const root = path.resolve(__dirname, '..');
const sourcePath = path.join(root,'fixtures/source/src/clipboard-blurhash.tsx');
const original = fs.readFileSync(sourcePath);
assert.equal(crypto.createHash('sha256').update(original).digest('hex'),'9ec38753704ce821ad18dc44a76dcff4a62038a67e4f5597ae1e035fec71e46e'); // pinned below by provenance generator
const sandboxRoot = path.join(root,'.work','source-oracle');
fs.mkdirSync(sandboxRoot,{recursive:true}); // the ONLY source-oracle real filesystem producer
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
function harness(platform='darwin') {
  const files = new Map(), log=[], effects=[], calls=[];
  let states=[], cursor=0;
  const os = {tmpdir:()=>sandboxRoot, homedir:()=>sandboxRoot};
  const fakeFs = Object.freeze({
    existsSync:p=>{log.push(["exists",p]);return files.has(p);},
    statSync:p=>({isFile:()=>files.get(p)?.regular!==false}),
    readdirSync:p=>{assert.equal(p,sandboxRoot);log.push(['readdir',p]);return ['blurhash-raw-foreign.rgba','blurhash-png-foreign.png','keep.txt'];},
    writeFileSync:(p,b)=>{assert.equal(path.dirname(p),sandboxRoot);log.push(['write',p,sha(b)]);files.set(p,{bytes:Buffer.from(b)});},
    readFileSync:p=>{assert.equal(path.dirname(p),sandboxRoot);log.push(['read',p]);return files.get(p)?.bytes??Buffer.from('fake-png');},
    unlinkSync:p=>{assert.equal(path.dirname(p),sandboxRoot);log.push(['unlink',p]);files.delete(p);},
  });
  const callable = type => Object.assign(props=>({type,props}),{Metadata:null});
  const Detail=callable('Detail');Detail.Metadata=callable('Detail.Metadata');Detail.Metadata.Label=callable('Detail.Metadata.Label');Detail.Metadata.Separator=callable('Detail.Metadata.Separator');
  const Action=callable('Action');Action.CopyToClipboard=callable('Action.CopyToClipboard');Action.OpenInBrowser=callable('Action.OpenInBrowser');
  const List=callable('List');List.Item=callable('List.Item');List.EmptyView=callable('List.EmptyView');
  let clip={}, selection=[];
  const api={Detail,ActionPanel:callable('ActionPanel'),Action,List,Icon:{Image:'Image',Clipboard:'Clipboard',Document:'Document'},Toast:{Style:{Animated:1,Success:2,Failure:3}},
    showToast:async x=>calls.push(['toast',x]),Clipboard:{read:async()=>{calls.push(['clipboard']);return clip;}},getSelectedFinderItems:async()=>{calls.push(['selection']);return selection;},open:async x=>calls.push(['open',x])};
  let dimensions={width:8,height:6};
  let locatorMissing=false;
  const child={execFileSync:(binary,args,opts)=>{
    log.push(['exec',binary,args,opts]);
    if((binary==='which'||binary==='where.exe')&&locatorMissing)throw new Error('fake missing');
    if(binary==='which'||binary==='where.exe')return '/fake/'+args[0]+'\n';
    if(binary.includes('ffprobe'))return JSON.stringify({streams:[{codec_type:'audio'},dimensions]});
    if(args.includes('pipe:1')) {const size=args[args.indexOf('-s')+1].split('x').map(Number);return Buffer.from(Array.from({length:size[0]*size[1]*4},(_,i)=>(i*73+29)%256));}
    const dest=args.at(-1);assert.equal(path.dirname(dest),sandboxRoot);files.set(dest,{bytes:Buffer.from('fake-png')});return Buffer.alloc(0);
  }};
  const React={useEffect:f=>effects.push(f),useState:initial=>{const i=cursor++;if(!(i in states))states[i]=initial;return [states[i],v=>states[i]=v];},createElement:(type,props,...children)=>({type,props:{...props,children}})};
  const allow={'@raycast/api':api,react:React,'node:child_process':child,blurhash:library,path:path.posix,fs:fakeFs,os};
  const context=vm.createContext({Buffer,Uint8ClampedArray,Date:{now:()=>12345},process:Object.freeze({platform,env:Object.freeze({})}),exports:{},require:name=>{assert.ok(Object.hasOwn(allow,name),'uncaptured import '+name);return allow[name];}});
  const js=ts.transpileModule(original.toString()+'\nexport const oracle={findBinary,ffmpegInstall,getDimensions,getPixelData,generatePreview,generateBlurHash,resolveClipboardFile,hasSupportedExt,aspectRatio,ResultView,FallbackMenu,cleanupStaleTempFiles};',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS,jsx:ts.JsxEmit.React,jsxFactory:'React.createElement',esModuleInterop:true}}).outputText;
  // JSX factory is an explicit controlled value, never the user's installed API.
  context.React=React;
  new vm.Script(js,{filename:'unchanged-source.tsx'}).runInContext(context,{timeout:2000});
  return {o:context.exports.oracle,files,log,calls,setClip:x=>clip=x,setSelection:x=>selection=x,setDimensions:x=>dimensions=x,setLocatorMissing:x=>locatorMissing=x,
    run:async()=>{cursor=0;const tree=context.exports.default();for(const f of effects.splice(0))f();for(let n=0;n<5;n++)await Promise.resolve();return tree;},render:()=>{cursor=0;return context.exports.default();}};
}
(async()=>{
  // Tripwire covers any accidental source access to a real producer after imports.
  // These methods are restored only before the harness writes its own evidence file.
  const realChild=require('node:child_process'),restore=[];
  for(const [object,names] of [[fs,['writeFileSync','unlinkSync','rmSync','rmdirSync','mkdirSync','renameSync','appendFileSync','createWriteStream','openSync']],[realChild,['execFileSync','execSync','exec','execFile','spawn','spawnSync','fork']]])for(const name of names){const original=object[name];restore.push(()=>object[name]=original);object[name]=()=>{throw new Error('real boundary tripwire: '+name);};}

  const vectors=[];
  for(const [w,h] of [[1,1],[2,3],[8,6],[100,1],[1,100],[99,100],[100,100],[17,11]]) {
    for(const mode of ['black','white','red','green','blue','pattern','alpha','gray']) {
      const pixels=Array.from({length:w*h*4},(_,i)=>{
        const c=i%4;if(c===3)return mode==='alpha'?(i*17)%256:255;
        if(mode==='black')return 0;if(mode==='white')return 255;if(mode==='red')return c===0?255:0;if(mode==='green')return c===1?255:0;if(mode==='blue')return c===2?255:0;
        if(mode==='gray')return 128;return (i*73+29)%256;
      });
      const hash=library.encode(new Uint8ClampedArray(pixels),w,h,4,3);
      const preview=library.decode(hash,37,29);
      vectors.push({name:`${w}x${h}-${mode}`,w,h,pixels,hash,decodeWidth:37,decodeHeight:29,decoded:Array.from(preview)});
    }
  }
  // Deterministic random images and alpha invariance, real JS library only.
  let seed=0xdeadbeef;
  for(let i=0;i<64;i++) {
    const w=1+(i*17)%100,h=1+(i*13)%100;
    const pixels=Array.from({length:w*h*4},()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed>>>24;});
    const hash=library.encode(new Uint8ClampedArray(pixels),w,h,4,3);
    vectors.push({name:'random-'+i,w,h,pixels,hash,decodeWidth:7,decodeHeight:11,decoded:Array.from(library.decode(hash,7,11))});
  }
  const h=harness(),o=h.o;
  h.files.set('/image.PNG',{});h.files.set('/encoded name.png',{});h.files.set('/literal%20name.png',{});h.files.set('/extensionless',{});h.files.set('/folder.png',{regular:false});
  const resolutions=[];
  for(const clip of [{file:'/extensionless'},{text:'/extensionless'},{text:'\uFEFF file:///encoded%20name.png \u00a0'},{text:'/literal%20name.png'},{text:'/folder.png'},{text:'/image.PNG'},{text:'file:///bad%xx.png'},{file:'file:///bad%FF'}]) {
    let result,error;try{result=o.resolveClipboardFile(clip);}catch(e){error=e.message;}
    resolutions.push({clip,result,error});
  }
  assert.equal(resolutions[0].result,'/extensionless');assert.equal(resolutions[1].result,null);assert.equal(resolutions[2].result,'/encoded name.png');assert.ok(resolutions[6].error);
  for(const extension of ['mp4','mov','avi','mkv','webm','m4v','flv','wmv','3gp','ts','jpg','jpeg','png','gif','bmp','webp','tiff','tif','heic','heif','avif','ico'])assert.ok(o.hasSupportedExt('/file.'+extension.toUpperCase()));
  assert.equal(o.hasSupportedExt('/file.svg'),false);
  const result=o.generateBlurHash('/image.PNG');
  assert.equal(result.copyText,`image.PNG 8x6 4:3 ${result.hash}`);
  const pv=o.ResultView({result});assert.equal(pv.props.metadata.props.children.length,5); // component functions are controlled fixtures
  // Execute source cleanup and preview callbacks against fakes, including foreign names.
  o.cleanupStaleTempFiles();
  assert.ok(h.log.some(x=>x[0]==='readdir'));assert.equal(h.log.filter(x=>x[0]==='unlink'&&x[1].includes('foreign')).length,2);
  assert.equal(fs.readdirSync(sandboxRoot).length,0,'source callbacks never wrote real files');
  assert.throws(()=>harness().o.resolveClipboardFile({text:'%'}));
  h.setClip({text:'/image.PNG'});await h.run();assert.ok(h.calls.some(x=>x[0]==='clipboard'));
  const fallback=harness();fallback.setClip({});fallback.setSelection([]);await fallback.run();const fb=fallback.render();assert.ok(fb);
  const menu=fallback.o.FallbackMenu({onResult:()=>{}});const rows=menu.props.children;const history=rows[1].props.actions.props.children[0];await history.props.onAction();assert.equal(fallback.calls.at(-1)[1],'raycast://extensions/raycast/clipboard-history/clipboard-history');
  await rows[2].props.actions.props.children[0].props.onAction();assert.ok(fallback.calls.some(x=>x[0]==='toast'&&x[1].title==='Could not use selected file'));
  const win=harness('win32');assert.equal(win.o.findBinary('ffmpeg'),'/fake/ffmpeg.exe');assert.equal(win.o.ffmpegInstall().command,'winget install --id Gyan.FFmpeg');
  const zero=harness();zero.setDimensions({width:1000,height:1});assert.throws(()=>zero.o.generateBlurHash('/image.png'));
  const missing=harness();missing.setLocatorMissing(true);await missing.run();const missingView=missing.render();assert.ok(missingView.props.markdown.includes('brew install ffmpeg'));assert.equal(missing.calls.filter(x=>x[0]==='clipboard').length,0);
  const sel=harness();sel.files.set('/bad.txt',{});sel.files.set('/good.PNG',{});sel.setSelection([{path:'/bad.txt'},{path:'/good.PNG'}]);let selectedResult;
  const selMenu=sel.o.FallbackMenu({onResult:r=>selectedResult=r});await selMenu.props.children[2].props.actions.props.children[0].props.onAction();assert.equal(selectedResult.fileName,'good.PNG');
  const malformed=harness();malformed.setClip({text:'%'});await malformed.run();assert.ok(malformed.calls.some(x=>x[0]==='toast'&&x[1].title==='Error'));
  const utilitySource=fs.readFileSync(path.join(root,'fixtures/library/typescript/utils.ts'),'utf8');const utilContext=vm.createContext({exports:{}});new vm.Script(ts.transpileModule(utilitySource,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText).runInContext(utilContext);
  const util=utilContext.exports;
  const srgbValues=[-100,-1,0,0.0031308-Number.EPSILON,0.0031308,0.0031308+Number.EPSILON,1,2,100];
  for(let b=0;b<255;b++){const target=(b+0.5)/255;const linear=target<=0.04045?target/12.92:Math.pow((target+0.055)/1.055,2.4);for(const delta of [-Number.EPSILON,0,Number.EPSILON])srgbValues.push(linear+delta);}
  const srgb=srgbValues.map(value=>({value,valueBits:(()=>{const b=Buffer.alloc(8);b.writeDoubleBE(value);return b.toString("hex");})(),byte:util.linearTosRGB(value)}));
  restore.reverse().forEach(f=>f());
  fs.writeFileSync(path.join(root,'fixtures','oracle.json'),JSON.stringify({sourceSha256:sha(original),srgb,vectors:vectors.map(({pixels,decoded,...v})=>({...v,pixelsBase64:Buffer.from(pixels).toString('base64'),decodedBase64:Buffer.from(decoded).toString('base64')})),resolutions,result,previewRgbaSha256:h.log.find(x=>x[0]==='write')[2],helperCalls:JSON.parse(JSON.stringify(h.log.filter(x=>x[0]==='exec')).split(sandboxRoot).join('<PRIVATE_ORACLE>')),boundary:{allImportsCaptured:true,realSourceWrites:0,fakeCleanupForeignDeletes:2,realDesktopCalls:0}},null,2)+'\n');
  console.log(`safe source oracle: ${vectors.length} codec vectors, acquisition/actions/cleanup/preview seams passed`);
})().catch(e=>{console.error(e);process.exitCode=1;});
