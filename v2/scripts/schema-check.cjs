'use strict';
// Execute the genuine pinned JSON Schema via Ajv 2020, separately from host runtime validator.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const Ajv=require('../.work/schema/node_modules/ajv/dist/2020');const formats=require('../.work/schema/node_modules/ajv-formats');
const root=path.resolve(__dirname,'..');const ajv=new Ajv({strict:false,allErrors:true});formats(ajv);
const validate=ajv.compile(JSON.parse(fs.readFileSync(path.join(root,'fixtures/host/schemas/extension-v3.schema.json'))));const manifest=JSON.parse(fs.readFileSync(path.join(root,'extension.json')));assert.ok(validate(manifest),JSON.stringify(validate.errors));
for(const [field,value]of [['schemaVersion',2],['runtime','javascript'],['execution','bun'],['id','com.tezbar.blurhash'],['replaces',['anything']],['capabilities',['process.exec']],['artifacts',{'x86_64-unknown-linux-gnu':{}}]]){const bad=structuredClone(manifest);bad[field]=value;assert.equal(validate(bad),false,field);}
assert.deepEqual(manifest.capabilities,['clipboard.read','clipboard.write','selection.read','shell.open']);assert.equal(manifest.replaces.join(','),'com.tezbar.blurhash,raycast.blurhash');assert.deepEqual(manifest.artifacts,{});
console.log('genuine schema 3: positive canonical identity/replacements/minimal grants/artifacts:{} and 7 negatives passed');
