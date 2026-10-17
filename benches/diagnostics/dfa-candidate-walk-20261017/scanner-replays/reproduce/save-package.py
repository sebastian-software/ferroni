from pathlib import Path
import sys,shutil,json,hashlib
root=Path(sys.argv[1]);out=Path(sys.argv[2]);label=sys.argv[3];pkg=root/'node/ferriki';target=out/label
out.mkdir(parents=True,exist_ok=True);target.mkdir(exist_ok=True)
for name in ['index.mjs','native.mjs','platforms.mjs','transformers.mjs','classes.mjs','package.json','.benchmark-build.json']:
 shutil.copy2(pkg/name,target/name)
for name in ['src','assets']:shutil.copytree(pkg/name,target/name,dirs_exist_ok=True)
side=target/'node_modules/@ferriki/darwin-arm64';side.mkdir(parents=True,exist_ok=True)
for name in ['package.json','ferriki.node']:shutil.copy2(root/'node/platforms/darwin-arm64'/name,side/name)
shutil.copy2(side/'ferriki.node',out/f'{label}.node');shutil.copy2(pkg/'.benchmark-build.json',out/f'{label}-build.json')
r=json.loads((pkg/'.benchmark-build.json').read_text());assert hashlib.sha256((side/'ferriki.node').read_bytes()).hexdigest()==r['binarySha256']
print('Saved',target,flush=True)
