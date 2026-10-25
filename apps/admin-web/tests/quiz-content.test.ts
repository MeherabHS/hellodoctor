import test from 'node:test';
import assert from 'node:assert/strict';
import {emptyQuiz,newQuestion,quizErrors,questionLanguage,readQuizFile,csvCell,quizTemplate} from '../src/lib/quizContent.ts';

test('MCQs default OFF; incomplete drafts are distinct from review-ready inclusion',()=>{
  assert.equal(emptyQuiz().include_quiz,false);const question=newQuestion();const quiz={include_quiz:false,questions:[question]};
  assert.deepEqual(quizErrors(quiz),[]);assert.ok(quizErrors(quiz,true).length>0);
  question.text_en='Question';question.explanation_en='Because';question.options[0].text_en='First';question.options[1].text_en='Second';question.correct_option_id=question.options[0].id;
  assert.deepEqual(quizErrors(quiz,true),[]);assert.equal(questionLanguage(question,'sw').fallback,true);
  question.text_sw='Swali';assert.ok(quizErrors(quiz,true).some(e=>e.field==='translation'));
  question.explanation_sw='Maelezo';question.options.forEach(o=>{o.text_sw='Jibu';});assert.deepEqual(quizErrors(quiz,true),[]);assert.equal(questionLanguage(question,'sw').text,'Swali');
});
test('MCQ validation rejects duplicate questions/IDs, excess counts and cross-question answers',()=>{
  const first=newQuestion();first.text_en='Same question';const second=newQuestion();second.text_en=' SAME   QUESTION ';
  second.options[0].id=first.options[0].id;second.correct_option_id=first.options[1].id;
  const errors=quizErrors({include_quiz:true,questions:[first,second]});assert.ok(errors.some(e=>e.message.includes('Duplicate')));assert.ok(errors.some(e=>e.field==='options.id'));assert.ok(errors.some(e=>e.field==='correct_option_id'));
  assert.ok(quizErrors({include_quiz:false,questions:Array.from({length:51},newQuestion)}).some(e=>e.field==='questions'));
});
test('MCQ import files enforce format, byte size and strict UTF-8 before transport',async()=>{
  const file=new File(['{}'],'quiz.JSON',{type:'application/json'});assert.deepEqual(await readQuizFile(file),{format:'json',content:'{}'});
  for(const bad of [new File(['x'],'questions.exe'),new File([],'empty.csv'),new File(['x'.repeat(128*1024+1)],'large.csv'),new File([new Uint8Array([0xff,0xfe])],'invalid.json')])await assert.rejects(readQuizFile(bad));
});
test('import templates are versioned and CSV cells neutralize spreadsheet formulas',()=>{
  const json=JSON.parse(quizTemplate('json'));assert.equal(json.schema_version,1);assert.equal(json.questions[0].correct_option,0);
  assert.ok(quizTemplate('csv').includes('correct_option'));assert.equal(csvCell('=1+1'),'"\'=1+1"');assert.equal(csvCell(' @SUM(A1)'),'"\' @SUM(A1)"');assert.equal(csvCell('quoted "text"'),'"quoted ""text"""');
});
