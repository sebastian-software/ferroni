import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const [ferrikiPath, resultPath] = process.argv.slice(2);
const root = resolve(resultPath);
const { compareReports, readReport } = await import(pathToFileURL(join(resolve(ferrikiPath),'node/scripts/tiobe-benchmark.mjs')));
const cases=[];const comparisons=[];
for (const language of ['astro','scss','bash']) {
 const reports=Object.fromEntries(['control','candidate'].map(v => [v,[1,2].map(n => readReport(join(root,`focused-${language}-${v}-${n}.json.gz`)))]));
 for (const runs of Object.values(reports)) for (const report of runs) {
  assert.equal(report.languages.length,1);assert.equal(report.languages[0].status,'measured');
  assert.equal(report.revision.status,'');assert.equal(report.nativeBuild.ferroni.revision.status,'');
  for (const c of report.languages[0].cases) for (const [id,r] of Object.entries(c.results)) {
   if (id==='prism' && !report.languages[0].prism) {assert.equal(r.status,'unsupported');continue;}
   assert.equal(r.status,'ok');assert.equal(r.validation.sourcePreserved,true);
   if (id!=='prism')assert.deepEqual(r.validation.referenceParity,{tokens:true,html:true});
  }
 }
 for (const [kind,a,b] of [['pair-1',reports.control[0],reports.candidate[0]],['pair-2',reports.control[1],reports.candidate[1]],['control-repeat',...reports.control],['candidate-repeat',...reports.candidate]]) {
  const comparison=compareReports(a,b);assert.equal(comparison.comparisons.length,4);assert.deepEqual(comparison.excluded,[]);comparisons.push({language,kind,...comparison});
 }
 for (const size of ['example','large']) {
  const values=Object.fromEntries(Object.entries(reports).map(([variant,runs])=>[variant,runs.map(r=> {
   const c=r.languages[0].cases.find(c=>c.size===size);
   const best=['shiki-wasm','shiki-js'].sort((a,b)=>c.results[a].html.medianMs-c.results[b].html.medianMs)[0];
   return {ferrikiHtmlMs:c.results.ferriki.html.medianMs,bestShikiHtmlMs:c.results[best].html.medianMs,bestShikiHtmlEngine:best};
  })]));
  cases.push({language,size,...values,pairedHtmlChangePercent:[0,1].map(n=>100*(values.candidate[n].ferrikiHtmlMs/values.control[n].ferrikiHtmlMs-1)),
   pairedRelativeToBestShikiChangePercent:[0,1].map(n=>100*((values.candidate[n].ferrikiHtmlMs/values.candidate[n].bestShikiHtmlMs)/(values.control[n].ferrikiHtmlMs/values.control[n].bestShikiHtmlMs)-1))});
 }
}
const summary={boundary:'Focused ABBA repeats of initial small Astro, SCSS, and Bash timing increases; same ordinary addons, fixtures, and public method',order:['control-1','candidate-1','candidate-2','control-2'],comparisons,cases};
writeFileSync(join(root,'focused-summary.json'),JSON.stringify(summary,null,2)+'\n');
console.log(JSON.stringify(cases,null,2));
