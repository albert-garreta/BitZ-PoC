import fs from 'node:fs/promises';
import assert from 'node:assert/strict';
import { Workbook, SpreadsheetFile } from '@oai/artifact-tool';

const outputDir = new URL('.', import.meta.url).pathname;
const attachment = '/Users/johnwu/.codex/attachments/59904aee-f6e4-42db-af2f-4457a072934b/pasted-text.txt';
const evidence = '/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead';
const text = await fs.readFile(attachment, 'utf8');
const original = text.split('\n').filter(x => /^\| 1\//.test(x)).map(line =>
  line.split('|').slice(1,-1).map((x,i) => i===0 || i===2 ? x.trim() : Number(x.trim().replaceAll(',',''))));
assert.equal(original.length,48);
const records = JSON.parse(await fs.readFile(`${evidence}/analysis.json`,'utf8')).native.filter(x=>x.family==='binius-focused');
const metrics = ['setup_ms','witness_ms','commit_ms','piop_ms','opening_ms','witness_to_proof_ms','verify_ms','proof_bytes','peak_rss_bytes'];
const workbook = Workbook.create();
const main = workbook.worksheets.add('Benchmarks');
const detail = workbook.worksheets.add('PCS parameters');
const first = 10;
const detailRows = [];
const mappings = [];
let levelCount = 0;

for (const row of original) {
  const backend = row[2].endsWith('BaseFold') ? 'binius64' : 'binius64-ligerito';
  const record = records.find(x=>x.rate===row[0] && x.log_multiplications===row[1] && x.backend===backend);
  assert(record, `Missing source for ${row.slice(0,3)}`);
  for(let i=0;i<metrics.length;i++) {
    let n=record[metrics[i]];
    if(i===7) n/=1000;
    if(i===8) n/=2**30;
    assert(Math.abs(row[i+3]-n)<=0.005001, `Pasted measurement mismatch: ${row.slice(0,3)} ${metrics[i]}`);
  }
  const cfg = record.config;
  const start = first+detailRows.length;
  const source = `Focused campaign albert-requested-_f62mead; ${record.source}/samples.jsonl; config; e=${row[1]}`;
  const map = {record,start,oracles:[]};
  if(backend==='binius64') {
    detailRows.push([row[0],row[1],row[2],'Batched PCS','FRI queries',null,row[0],cfg.fri_queries,cfg.fri_grinding_bits,null,null,null,
      'Unique decoding',null,cfg.fri_query_target_bits,null,null,null,null,'SHA-256','FRI query phase only',null,
      'OOD parameter not reported for this backend',null,source]);
  } else {
    assert.equal(cfg.oracles.length,2);
    assert.equal(cfg.oracles[0].configuration.levels[0].queries,cfg.oracles[1].configuration.levels[0].queries);
    for(let oracle=0;oracle<cfg.oracles.length;oracle++) {
      const o=cfg.oracles[oracle];
      const c=o.configuration;
      const name=oracle===0?'Witness':'IntMul pushforward';
      const round0 = first+detailRows.length;
      detailRows.push([row[0],row[1],row[2],name,'Round 0',null,row[0],null,null,null,o.ood_grinding_bits,1,
        'Johnson OOD',0.02,108,cfg.whole_protocol_bits,c.log_n,2**c.initial_k,null,'BLAKE3','Round-0 modeled target',null,
        'One packed-message OOD evaluation claim per oracle',null,source+'; Round 0 also confirmed in source']);
      const levels=[];
      for(let l=0;l<c.levels.length;l++) {
        const level=c.levels[l];
        levels.push(first+detailRows.length);
        detailRows.push([row[0],row[1],row[2],name,'Ligerito level',l,`1/${2**level.log_inv_rate}`,level.queries,
          level.grinding_bits,level.fold_grinding_bits,null,level.ood_samples,'Johnson OOD',level.eta,
          level.target_security_bits,cfg.whole_protocol_bits,c.log_n,2**c.initial_k,level.k_recursive,'BLAKE3',
          'Component target incl. grinding',null,
          l===0?'Level-0 recursive OOD is zero; separate Round 0 is above':'Recorded recursive OOD sample count',null,source]);
        levelCount++;
      }
      map.oracles.push({round0,levels});
    }
  }
  map.end = first+detailRows.length-1;
  mappings.push(map);
}
assert.equal(levelCount,189);
assert.equal(detailRows.length,261);

const mainHeaders = ['Rate','e','Backend','Setup (ms)','Witness (ms)','Commit (ms)','PIOP (ms)','Opening (ms)',
  'Witness to proof (ms)','Verify (ms)','Proof (KB)','RSS (GiB)','Initial PCS queries','Query grind (bits)',
  'Initial fold grind: witness (bits)','Initial fold grind: IntMul (bits)','Round-0 grind: witness (bits)',
  'Round-0 grind: IntMul (bits)','Round-0 OOD claims','Recursive OOD samples','Total OOD claims','Hash','Security scope'];
const detailHeaders = ['Initial rate','e','Backend','Oracle','Stage','Level','Level rate','Queries','Query grind (bits)',
  'Initial fold grind (bits)','OOD grind (bits)','OOD count','Proximity regime','Eta','Target (bits)',
  'Modeled composition (bits)','Oracle log2 size','Initial lanes','Recursive k','Hash','Target scope',null,'Notes',null,'Source'];

function setup(sheet,lastCol,lastRow,widths) {
  sheet.showGridLines=false;
  const used=sheet.getRangeByIndexes(0,0,lastRow,lastCol);
  used.format.font={name:'Arial',size:10,color:'#202A35'};
  used.format.verticalAlignment='center';
  used.format.rowHeight=21;
  used.format.wrapText=false;
  for(let c=0;c<widths.length;c++) sheet.getRangeByIndexes(0,c,lastRow,1).format.columnWidthPx=widths[c];
  sheet.getRange('A2').format.font={name:'Arial',size:15,bold:true,color:'#1F3653'};
  sheet.getRangeByIndexes(2,0,1,lastCol).format.borders={bottom:{style:'thin',color:'#BDC9D6'}};
  sheet.freezePanes.freezeRows(9);
  sheet.freezePanes.freezeColumns(3);
}
setup(main,23,57,[72,48,190,104,104,104,104,104,124,104,105,94,112,112,144,144,144,144,125,130,120,98,350]);
setup(detail,21,270,[85,48,190,142,125,65,85,92,110,125,110,88,135,65,110,150,112,105,98,90,340]);
main.tabColor='#1F3653';
main.getRange('A2').values=[['Binius focused benchmarks']];
main.getRange('A4').values=[['48 pasted observations. Timings in ms; proof KB is decimal; RSS GiB is binary. Five measured repetitions per case.']];
main.getRange('A5').values=[['Source: supplied binius-focused table, 13 September 2026. Original rounded measurements are preserved.']];
main.getRange('A6').values=[['Initial PCS queries: batched FRI queries for BaseFold; per-oracle level-0 queries for Ligerito. They are configured counts, not unique paths.']];
main.getRange('A7').values=[['Ligerito grinding difficulty is in bits. Initial fold difficulty can taper within a level. OOD totals count claims/samples, not challenge coordinates.']];
main.getRange('A8').values=[['Timing varied under CPU/memory contention. n.a. means no corresponding reported parameter; it is not zero. W and IntMul are separate oracles.']];
detail.getRange('A2').values=[['PCS parameters by oracle and level']];
detail.getRange('A4').values=[['24 BaseFold configurations, 48 Ligerito Round-0 records, and 189 Ligerito recursive-level records.']];
detail.getRange('A5').values=[['Sources: albert-requested-_f62mead/binius-pcs/{rate1,rate2,rate3}/samples.jsonl (config). Round 0: binary_pcs.rs and ligerito_flock.rs.']];
detail.getRange('A6').values=[['One Round-0 OOD claim per Ligerito oracle. Level-0 recursive OOD is zero and does not remove that separate claim.']];
detail.getRange('A7').values=[['Queries are per level and oracle. Repeated level queries may share paths; summing them is not a count of unique openings.']];
detail.getRange('A8').values=[['Blank parameters are not applicable or not reported. Initial fold grind is the recorded starting difficulty, not a sum over folding rounds.']];
main.getRange('A9:W9').values=[mainHeaders];
main.getRange('A10:L57').values=original;
detail.getRange('A9:U9').values=[detailHeaders.slice(0,21)];
detail.getRange('A10:U270').values=detailRows.map(row=>row.slice(0,21));
main.getRange('D10:L57').setNumberFormat('#,##0.00');
main.getRange('B10:B57').setNumberFormat('0');
main.getRange('M10:U57').setNumberFormat('#,##0');
detail.getRange('B10:B270').setNumberFormat('0');
detail.getRange('F10:F270').setNumberFormat('0');
detail.getRange('H10:L270').setNumberFormat('#,##0');
detail.getRange('N10:N270').setNumberFormat('0.00');
detail.getRange('O10:O270').setNumberFormat('0');
detail.getRange('P10:P270').setNumberFormat('0.000000');
detail.getRange('Q10:S270').setNumberFormat('#,##0');

for(let i=0;i<mappings.length;i++) {
  const map=mappings[i], r=first+i;
  if(map.oracles.length===0) {
    main.getRange(`M${r}:N${r}`).formulas=[[`='PCS parameters'!H${map.start}`,`='PCS parameters'!I${map.start}`]];
    main.getRange(`O${r}:U${r}`).values=[Array(7).fill('n.a.')];
    main.getRange(`V${r}:W${r}`).values=[['SHA-256','FRI query phase only; target 100 bits']];
  } else {
    const w=map.oracles[0], im=map.oracles[1];
    main.getRange(`M${r}:R${r}`).formulas=[[
      `='PCS parameters'!H${w.levels[0]}`,`='PCS parameters'!I${w.levels[0]}`,
      `='PCS parameters'!J${w.levels[0]}`,`='PCS parameters'!J${im.levels[0]}`,
      `='PCS parameters'!K${w.round0}`,`='PCS parameters'!K${im.round0}`]];
    main.getRange(`S${r}`).formulas=[[`=SUM('PCS parameters'!L${w.round0},'PCS parameters'!L${im.round0})`]];
    main.getRange(`T${r}`).formulas=[[`=SUMIFS('PCS parameters'!L${map.start}:L${map.end},'PCS parameters'!E${map.start}:E${map.end},"Ligerito level")`]];
    main.getRange(`U${r}`).formulas=[[`=SUM(S${r}:T${r})`]];
    main.getRange(`V${r}:W${r}`).values=[['BLAKE3','Modeled composition including grinding; ≥100 bits']];
  }
}

function styleTable(sheet,range,name,lastRow,lastColumn) {
  const table=sheet.tables.add(range,true,name);
  table.style='TableStyleMedium2';
  table.showFilterButton=true;
  const header=sheet.getRangeByIndexes(8,0,1,lastColumn);
  header.format.fill='#1F3653';
  header.format.font={name:'Arial',size:10,bold:true,color:'#FFFFFF'};
  header.format.horizontalAlignment='center';
  header.format.wrapText=true;
  header.format.rowHeightPx=58;
  header.format.borders={insideVertical:{style:'thin',color:'#FFFFFF'}};
  sheet.getRangeByIndexes(9,0,lastRow-9,lastColumn).format.rowHeightPx=26;
}
styleTable(main,'A9:W57','BenchmarkObservations',57,23);
styleTable(detail,'A9:U270','PCSConfiguration',270,21);
main.getRange('D10:U57').format.horizontalAlignment='right';
detail.getRange('F10:L270').format.horizontalAlignment='right';
detail.getRange('N10:S270').format.horizontalAlignment='right';
main.getRange('V10:V57').format.horizontalAlignment='center';
detail.getRange('T10:T270').format.horizontalAlignment='center';
main.getRange('A4:A8').format.font={name:'Arial',size:10,color:'#596779'};
detail.getRange('A4:A8').format.font={name:'Arial',size:10,color:'#596779'};
workbook.recalculate();

// Verify every supplied value and every calculated OOD summary against the source.
assert.deepEqual(main.getRange('A10:L57').values, original);
for(let i=0;i<mappings.length;i++) {
  const cfg=mappings[i].record.config;
  const row=main.getRangeByIndexes(9+i,12,1,9).values[0];
  if(cfg.oracles) {
    assert.equal(row[0],cfg.oracles[0].configuration.levels[0].queries);
    assert.equal(row[6],2);
    const total=cfg.oracles.reduce((a,o)=>a+o.configuration.levels.reduce((b,l)=>b+l.ood_samples,0),0);
    assert.equal(row[7],total);
    assert.equal(row[8],total+2);
  } else { assert.equal(row[0],cfg.fri_queries); assert.equal(row[1],0); }
}
console.log((await workbook.inspect({kind:'table',range:'Benchmarks!M9:U13',include:'values,formulas',tableMaxRows:5,tableMaxCols:9,maxChars:2000})).ndjson);
const errors=await workbook.inspect({kind:'match',searchTerm:'#REF!|#DIV/0!|#VALUE!|#NAME\\?|#N/A|#NUM!|#NULL!|#SPILL!|#CALC!',options:{useRegex:true,maxResults:100},summary:'Formula error scan',maxChars:1500});
console.log(errors.ndjson);
await fs.writeFile(`${outputDir}validation.json`,JSON.stringify({originalRowsPreserved:48,basefoldRecords:24,ligeritoRound0Records:48,ligeritoLevelRecords:189,allParameterSummariesChecked:true,formulaErrorScan:errors.ndjson},null,2));
for(const [sheetName,range,name] of [['Benchmarks','M9:W16','parameters-summary'],['PCS parameters','A1:L19','parameters-detail'],['PCS parameters','M9:U19','security-detail']]) {
  const preview=await workbook.render({sheetName,range,scale:1.5,format:'png'});
  await fs.writeFile(`${outputDir}${name}.png`,new Uint8Array(await preview.arrayBuffer()));
}
const output=await SpreadsheetFile.exportXlsx(workbook);
await output.save(`${outputDir}binius-focused-pcs-parameters.xlsx`);
console.log('Saved binius-focused-pcs-parameters.xlsx');
