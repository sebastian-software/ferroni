from pathlib import Path
import subprocess, hashlib
root=Path('/Users/sebastian/Workspace/ferriki')
out=Path('/private/tmp/ferroni-dfa-homepage-guard-20261018')
target=root/'node/scripts/bench-dfa-homepage-guard-20261018.mjs'
source=(out/'homepage-harness.mjs').read_bytes()
assert subprocess.check_output(['git','rev-parse','--path-format=absolute','--git-common-dir'],cwd=root,text=True).strip()==str(root/'.git')
assert subprocess.check_output(['git','rev-parse','--show-toplevel'],cwd=root,text=True).strip()==str(root)
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()=='ec10cb9b1d6c22f4fc701665d12a30394f81108f'
assert subprocess.check_output(['git','symbolic-ref','--short','HEAD'],cwd=root,text=True).strip()=='codex/borrow-inline-render'
assert not target.exists()
assert subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)==''
cmd=['node',str(target),'--engine','baseline=/private/tmp/ferroni-dfa-replay-20261017/consumer/baseline','--engine','candidate=/private/tmp/ferroni-dfa-replay-20261017/consumer/candidate','--corpus-root','/private/tmp/ferriki-style-ascii-20261017/published-corpus','--processes','32','--rounds','60','--warmups','12','--cold-runs','60','--ferroni-source-comparison','--write',str(out/'result.json')]
try:
    target.write_bytes(source)
    with (out/'stdout.json').open('w') as stdout, (out/'run.log').open('w') as stderr:
        result=subprocess.run(cmd,cwd=root/'node',stdout=stdout,stderr=stderr)
    (out/'exit-code.txt').write_text(str(result.returncode)+'\n')
    print('Homepage guard finished:',result.returncode,flush=True)
    if result.returncode:
        print((out/'run.log').read_text()[-8000:],flush=True)
    raise SystemExit(result.returncode)
finally:
    assert target.read_bytes()==source
    target.unlink()
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()=='ec10cb9b1d6c22f4fc701665d12a30394f81108f'
    assert subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)==''
