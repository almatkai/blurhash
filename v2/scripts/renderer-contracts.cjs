'use strict';
const fs=require('node:fs'),vm=require('node:vm'),path=require('node:path'),assert=require('node:assert/strict');
const ts=require('../.work/v1/node_modules/typescript');
const root=path.resolve(__dirname,'..');const host=path.join(root,'fixtures/host/src/renderer/src/raycast-api');
const React={createElement:(type,props,...children)=>({type,props:{...props,children}}),useState:initial=>[typeof initial==='function'?initial():initial,()=>{}],useEffect:()=>{},useMemo:f=>f(),useRef:value=>({current:value}),useCallback:f=>f,memo:f=>f};
const token=name=>props=>({type:name,props});
const calls=[];
const imports={react:React,'../../../shared/extensionRuntime':{},'../../colorConverter':{buildColorFormatRows:()=>[]},'../../ui/Markdown':{Markdown:token('Markdown')},'../../ui/primitives':{cx:()=>'',Hint:token('Hint'),HintBar:token('HintBar'),Kbd:token('Kbd'),ViewHeader:token('ViewHeader')},'./action-runtime':{Action:token('Action'),ActionPanel:token('ActionPanel'),ActionPanelOverlay:token('ActionPanelOverlay')},'./action-runtime-registry':{ActionRegistryContext:{Provider:token('Actions.Provider')}},'./form-runtime':{FormRuntime:token('FormRuntime')},'./grid-runtime':{GridRuntime:token('GridRuntime')},'./hooks':{NavigationContext:{Provider:token('Navigation.Provider')},setRuntimeContext:()=>{}}};
function compile(name,source,extra='') {
  const context=vm.createContext({exports:{},React,require:name=>{assert.ok(Object.hasOwn(imports,name),'uncaptured renderer import '+name);return imports[name];},window:{addEventListener:()=>{},removeEventListener:()=>{}},console});
  const code=ts.transpileModule(source+'\n'+extra,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS,jsx:ts.JsxEmit.React,jsxFactory:'React.createElement',esModuleInterop:true}}).outputText;
  new vm.Script(code,{filename:name}).runInContext(context,{timeout:2000});return context.exports;
}
function nodes(n,pred,out=[]) {if(!n||typeof n!=='object')return out;if(pred(n))out.push(n);for(const child of n.props?.children??[]) {if(Array.isArray(child))child.forEach(c=>nodes(c,pred,out));else nodes(child,pred,out);}return out;}
const detailSource=fs.readFileSync(path.join(host,'detail-runtime.tsx'),'utf8');
const before=compile('detail-before',detailSource,'export const contract={parseImageOnlyMarkdown,resolveMarkdownImageSrc};');
const after=compile('detail-after',detailSource.replace('onClick={onRunPrimaryAction}','onClick={() => onRunPrimaryAction()}'),'export const contract={parseImageOnlyMarkdown,resolveMarkdownImageSrc};');

function checkClick(detail) {
  imports['./detail-runtime']={DetailRuntime:detail.DetailRuntime};imports['./list-runtime']={ListRuntime:token('ListRuntime')};
  const runtime=compile('surface',fs.readFileSync(path.join(host,'index.tsx'),'utf8'));
  let received;const outer=runtime.ExtensionRuntimeSurface({title:'BlurHash',sessionId:'s',extensionId:'com.tezbar.native.blurhash',commandName:'clipboard-blurhash',root:{type:'Detail',props:{markdown:'![preview](data:image/png;base64,AA==)'}},actions:[{id:'7:copy-hash',title:'Copy BlurHash'}],onBack:()=>{},onInvokeAction:id=>received=id,onSearchTextChanged:()=>{},onLoadMore:()=>{}});
  const props=nodes(outer,n=>n.type===detail.DetailRuntime)[0].props;const tree=detail.DetailRuntime(props);const enter=nodes(tree,n=>n.type==='button'&&n.props.children?.includes('Enter'))[0];enter.props.onClick({type:'click'});return received;
}
imports['./detail-runtime']={DetailRuntime:before.DetailRuntime};imports['./list-runtime']={ListRuntime:token('ListRuntime')};
assert.equal(typeof checkClick(before),'object','frozen host forwards MouseEvent as action ID');assert.equal(checkClick(after),'7:copy-hash','generic wrapper forwards actual primary ID');
const png='data:image/png;base64,iVBORw0KGgo=';const img=after.contract.parseImageOnlyMarkdown(`![preview](${png})`,'');assert.equal(img.src,png);assert.equal(img.height,undefined);
const rootNode={type:'Detail',props:{markdown:`![preview](${png})`},metadata:{type:'Detail.Metadata',children:[{type:'Detail.Metadata.Label',props:{title:'File',text:'clipboard.png'}},{type:'Detail.Metadata.Label',props:{title:'BlurHash',text:'L00000'}}]}};
const tree=after.DetailRuntime({root:rootNode,title:'BlurHash',onBack:()=>{},onRunPrimaryAction:()=>{},onOpenActions:()=>{}});assert.equal(nodes(tree,n=>n.type==='img')[0].props.src,png);const metadata=nodes(tree,n=>n.type===after.MetadataSidebar)[0];assert.equal(metadata.props.root,rootNode.metadata);const label=after.MetadataItem({node:rootNode.metadata.children[0]});assert.ok(nodes(label,n=>n.type==='span').some(n=>n.props.children.includes('clipboard.png')));
const list=compile('list',fs.readFileSync(path.join(host,'list-runtime.tsx'),'utf8'),'export const contract={parseListRows};');const rows=list.contract.parseListRows({type:'List',children:[{type:'List.Item',props:{id:'selected',title:'Use Selected File',actionIds:['3:selected']}}]});assert.equal(rows[0].actionIds[0],'3:selected');
console.log('actual renderer: image data URI, metadata, List action IDs, Detail callback before/after passed (React/DOM layout is a seam; no GUI claim)');
