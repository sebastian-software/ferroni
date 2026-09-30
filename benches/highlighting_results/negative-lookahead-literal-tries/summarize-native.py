from pathlib import Path
import json
root=Path(__file__).resolve().parent
rows=[]
for bench,group in [('cpp_scanner_bench','cpp_scanner'),('scanner_compile_bench','scanner_compile_and_drop')]:
 for name in sorted({p.parent.name for p in (root/'criterion'/bench).glob('*/'+group+'/*/estimates.json')}):
  estimates={}
  for variant in ['control','candidate']:
   estimates[variant]=[]
   for n in [1,2]:
    e=json.loads((root/'criterion'/bench/(variant+'-'+str(n))/group/name/'estimates.json').read_text())
    estimates[variant].append({k:{'ms':v['point_estimate']/1e6,'ci95Ms':[v['confidence_interval']['lower_bound']/1e6,v['confidence_interval']['upper_bound']/1e6]} for k,v in e.items() if k in ['mean','median']})
  rows.append({'bench':bench,'group':group,'case':name,'estimates':estimates,'pairedMeanChangePercent':[100*(estimates['candidate'][n]['mean']['ms']/estimates['control'][n]['mean']['ms']-1) for n in [0,1]]})
report={'order':['control-1','candidate-1','candidate-2','control-2'],'criterion':{'samples':30,'warmupSeconds':1,'requestedMeasurementSeconds':3},'rows':rows}
(root/'native-summary.json').write_text(json.dumps(report,indent=2)+'\n')
for row in rows:print(row['case'],[round(e['mean']['ms'],3) for e in row['estimates']['control']],[round(e['mean']['ms'],3) for e in row['estimates']['candidate']],[round(v,2) for v in row['pairedMeanChangePercent']])
