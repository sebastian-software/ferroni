from pathlib import Path
import json,math,statistics,importlib.util,sys
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('comparison',ROOT/'compare-engines.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
BASE=json.loads((ROOT/'analysis/baseline.json').read_text())

def category(group,case):
 if group=='shared':return 'Shared document searches'
 if group=='oniguruma':return 'Advanced patterns'
 if case.startswith('compilation/') or '_compile_' in case:return 'Compilation'
 if group=='micro':return 'Short searches'
 if case.startswith('scanner_highlighting/'):return 'TextMate line scans'
 if case.startswith('scanner_documents/'):return 'Complete grammar documents'
 if case.endswith('/document'):return 'Scanner document replays'
 return 'Scanner group replays'

def geometric(xs):
 return math.exp(statistics.mean(math.log(v) for v in xs)) if xs else None

def analyze(allow_partial=False):
 current={};sources=[]
 for path in sorted((ROOT/'raw').rglob('measurements.json')):
  d=json.loads(path.read_text());profile=d['host']['runner_profile'];assert profile in BASE
  assert d['build']['git_commit']=='d791bf1b011f6e98619969872a4d7cb7161912a7',path
  assert not d['build']['git_status'],(path,d['build']['git_status'])
  sources.append({'file':str(path.relative_to(ROOT)),**{k:d[k] for k in ['host','build','settings']}})
  for group,cases in d['results'].items():
   assert group not in current.setdefault(profile,{}),(profile,group)
   assert set(cases)==set(module.CASES[group]),(profile,group,'missing cases',set(module.CASES[group])-set(cases))
   current[profile][group]={'cases':cases,**{k:d[k] for k in ['host','build','settings','notes']}}
 complete=len(sources)==8 and all(set(current.get(p,{}))==set(module.CASES) for p in BASE)
 if not complete and not allow_partial:
  raise SystemExit('Not all eight complete measurement artifacts are present yet.')
 rows=[];added=[];notes=[]
 for profile,groups in current.items():
  for group,new in groups.items():
   old=BASE[profile][group]
   assert old['build']['battle_inputs_sha256']==new['build']['battle_inputs_sha256'],(profile,group,'input revisions')
   for case,n in new['cases'].items():
    assert 'rust' in n['timing'],(profile,case)
    nv=n['timing']['rust']['mean_ns'];assert math.isfinite(nv) and nv>0
    ratios={e:t['mean_ns']/nv for e,t in n['timing'].items() if e!='rust'}
    row={'profile':profile,'group':group,'category':category(group,case),'case':case,'current_ns':nv,'current_engine_over_ferroni':ratios,'current_rust_ci_ns':n['timing']['rust'].get('mean_ci_ns')}
    if case not in old['cases']:
     added.append(row);continue
    b=old['cases'][case];bv=b['timing']['rust']['mean_ns'];ratio=nv/bv
    engine_changes={e:100*(1-t['mean_ns']/b['timing'][e]['mean_ns']) for e,t in n['timing'].items() if e in b['timing']}
    row.update(baseline_ns=bv,time_saved_percent=100*(1-ratio),baseline_rust_ci_ns=b['timing']['rust'].get('mean_ci_ns'),engine_time_saved_percent=engine_changes)
    row['reference_normalized_time_saved_percent']={e:100*(1-ratio/(n['timing'][e]['mean_ns']/b['timing'][e]['mean_ns'])) for e in ratios if e in b['timing']}
    row['baseline_engine_over_ferroni']={e:t['mean_ns']/bv for e,t in b['timing'].items() if e!='rust'}
    rows.append(row)
   notes.append({'profile':profile,'group':group,'baseline':old['notes'],'current':new['notes']})
 aggregates=[];standings=[]
 for profile in sorted(current):
  categories=sorted(set(r['category'] for r in rows if r['profile']==profile))
  for cat in categories:
   subset=[r for r in rows if r['profile']==profile and r['category']==cat]
   ratios=[r['current_ns']/r['baseline_ns'] for r in subset]
   normalized=[1-r['reference_normalized_time_saved_percent']['c']/100 for r in subset if 'c' in r['reference_normalized_time_saved_percent']]
   aggregates.append({'profile':profile,'category':cat,'common_cases':len(subset),'geometric_time_saved_percent':100*(1-geometric(ratios)),'c_normalized_cases':len(normalized),'c_normalized_time_saved_percent':100*(1-geometric(normalized)) if normalized else None,'raw_gains_at_least_5':sum(r['time_saved_percent']>=5 for r in subset),'raw_regressions_at_least_5':sum(r['time_saved_percent']<=-5 for r in subset)})
  for cat in categories:
   subset=[r for r in rows+added if r['profile']==profile and r['category']==cat]
   engines=sorted(set(e for r in subset for e in r['current_engine_over_ferroni']))
   for engine in engines:
    values=[r['current_engine_over_ferroni'][engine] for r in subset if engine in r['current_engine_over_ferroni']]
    standings.append({'profile':profile,'category':cat,'engine':engine,'timed_cases':len(values),'total_cases':len(subset),'geometric_engine_over_ferroni':geometric(values)})
 result={'complete':complete,'aggregates':aggregates,'rows':rows,'new_cases':added,'current_standings':standings,'provenance':sources,'engine_notes':notes}
 (ROOT/'analysis/comparison.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps({'common_rows':len(rows),'new_rows':len(added),'aggregates':aggregates},indent=2))
 return result

if __name__=='__main__':analyze('--partial' in sys.argv)
