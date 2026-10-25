export type QuizLanguage='en'|'sw';
export interface QuizOption{id:string;text_en:string;text_sw:string}
export interface QuizQuestion{id:string;text_en:string;text_sw:string;explanation_en:string;explanation_sw:string;options:QuizOption[];correct_option_id:string|null}
export interface QuizContent{include_quiz:boolean;questions:QuizQuestion[]}
export interface QuizRevision extends QuizContent{id:string;article_id:string;status:'DRAFT'|'PENDING_REVIEW'|'APPROVED'|'PUBLISHED';version:number;content_version:number;article_content_version:number;can_approve:boolean;requires_independent_review:boolean;approved_by:string|null}
export interface QuizDocument{schema_version:1;draft:QuizRevision|null;published:QuizRevision|null}
export interface QuizIssue{row:number;field:string;message:string}
export interface ImportPreview{questions:QuizQuestion[];errors:QuizIssue[]}
export const maxQuestions=50;
export const emptyQuiz=():QuizContent=>({include_quiz:false,questions:[]});
export const newOption=():QuizOption=>({id:crypto.randomUUID(),text_en:'',text_sw:''});
export const newQuestion=():QuizQuestion=>({id:crypto.randomUUID(),text_en:'',text_sw:'',explanation_en:'',explanation_sw:'',options:[newOption(),newOption()],correct_option_id:null});
export const quizRevision=(doc:QuizDocument)=>doc.draft||doc.published;
export const editableQuiz=(r:QuizContent):QuizContent=>({include_quiz:r.include_quiz,questions:r.questions.map(q=>({...q,options:q.options.map(o=>({...o}))}))});
export function quizErrors(quiz:QuizContent,complete=false):QuizIssue[]{
  const errors:QuizIssue[]=[];const add=(row:number,field:string,message:string)=>errors.push({row,field,message});
  if(quiz.questions.length>50||(complete&&!quiz.questions.length))add(0,'questions','Use 1–50 questions for an included quiz; excluded drafts may be empty.');
  const questionIds=new Set<string>(),optionIds=new Set<string>(),en=new Set<string>(),sw=new Set<string>();let bytes=0;
  const normalized=(s:string)=>s.trim().toLowerCase().replace(/\s+/g,' ');
  quiz.questions.forEach((q,i)=>{
    const row=i+1;if(!q.id||questionIds.has(q.id))add(row,'id','Question IDs must be unique.');questionIds.add(q.id);
    for(const [field,max,required] of [['text_en',2000,complete],['text_sw',2000,false],['explanation_en',4000,complete],['explanation_sw',4000,false]] as const){
      const text=q[field];bytes+=new TextEncoder().encode(text).length;
      if((required&&!text.trim())||Array.from(text).length>max||/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(text))add(row,field,'Text is missing, too long or contains prohibited control characters.');
    }
    for(const [text,seen] of [[q.text_en,en],[q.text_sw,sw]] as const){const key=normalized(text);if(key&&seen.has(key))add(row,'text','Duplicate question.');if(key)seen.add(key);}
    if(q.options.length>6||(complete&&q.options.length<2))add(row,'options','Use 2–6 options for review.');
    const hasSw=Boolean(q.text_sw.trim()||q.explanation_sw.trim()||q.options.some(o=>o.text_sw.trim()));
    q.options.forEach(o=>{if(!o.id||optionIds.has(o.id))add(row,'options.id','Option IDs must be unique.');optionIds.add(o.id);
      for(const lang of ['en','sw'] as const){const text=o[`text_${lang}`];bytes+=new TextEncoder().encode(text).length;if((complete&&(lang==='en'||hasSw)&&!text.trim())||Array.from(text).length>500||/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(text))add(row,`options.text_${lang}`,'Option text is missing, too long or contains prohibited control characters.');}
    });
    if(complete&&hasSw&&(!q.text_sw.trim()||!q.explanation_sw.trim()))add(row,'translation','Complete all Kiswahili fields or leave all blank for English fallback.');
    if((q.correct_option_id&&!q.options.some(o=>o.id===q.correct_option_id))||(complete&&!q.correct_option_id))add(row,'correct_option_id','Select one answer belonging to this question.');
  });
  if(bytes>256*1024)add(0,'questions','Total quiz text is limited to 256 KiB.');return errors;
}
export function questionLanguage(q:QuizQuestion,language:QuizLanguage){
  const sw=language==='sw'&&Boolean(q.text_sw.trim()&&q.explanation_sw.trim()&&q.options.every(o=>o.text_sw.trim()));
  return {text:sw?q.text_sw:q.text_en,explanation:sw?q.explanation_sw:q.explanation_en,options:q.options.map(o=>({id:o.id,text:sw?o.text_sw:o.text_en})),fallback:language==='sw'&&!sw};
}
export async function readQuizFile(file:File):Promise<{format:'csv'|'json';content:string}>{
  const format=file.name.toLowerCase().split('.').pop();
  if(!['csv','json'].includes(format||'')||!file.size||file.size>128*1024)throw new Error('Choose a nonempty CSV or JSON file up to 128 KiB.');
  let content:string;try{content=new TextDecoder('utf-8',{fatal:true}).decode(await file.arrayBuffer());}catch{throw new Error('Import files must contain valid UTF-8 text.');}
  return {format:format as 'csv'|'json',content};
}
export function csvCell(value:string){const safe=/^\s*[=+\-@]/.test(value)?`'${value}`:value;return `"${safe.replaceAll('"','""')}"`;}
export function quizTemplate(format:'csv'|'json'){
  if(format==='json')return JSON.stringify({schema_version:1,questions:[{text_en:'Which button saves a draft?',text_sw:'',options:[{text_en:'Save draft',text_sw:''},{text_en:'Publish now',text_sw:''}],correct_option:0,explanation_en:'Save draft keeps changes private.',explanation_sw:''}]},null,2);
  return [['text_en','text_sw','option_a_en','option_a_sw','option_b_en','option_b_sw','correct_option','explanation_en','explanation_sw'],['Which button saves a draft?','','Save draft','','Publish now','','a','Save draft keeps changes private.','']].map(row=>row.map(csvCell).join(',')).join('\r\n');
}
