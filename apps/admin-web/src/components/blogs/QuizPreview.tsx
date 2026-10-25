'use client';
import {useState} from 'react';
import {questionLanguage,type QuizContent,type QuizLanguage} from '../../lib/quizContent';

export function QuizPreview({content,language}:{content:QuizContent;language:QuizLanguage}){
  const [index,setIndex]=useState(0),[selected,setSelected]=useState<string|null>(null),[answered,setAnswered]=useState(false),[finished,setFinished]=useState(false);
  if(!content.include_quiz)return null;
  const question=content.questions[index];const sw=language==='sw';
  function restart(){setIndex(0);setSelected(null);setAnswered(false);setFinished(false);}
  return <section className="cms-quiz-preview" aria-label="Interactive quiz preview"><h2>{sw?'Maswali ya kujifunza':'Check your understanding'}</h2><p className="cms-hint">{sw?'Onyesho la Admin pekee. Majibu hayahifadhiwi.':'Admin simulation only. Responses are never saved.'}</p>
    {finished?<div role="status"><h3>{sw?'Onyesho limekamilika':'Preview complete'}</h3><p>{sw?'Hakuna matokeo ya Mgonjwa yaliyohifadhiwa.':'No Patient activity or results were recorded.'}</p><button type="button" onClick={restart}>{sw?'Anza tena':'Start again'}</button></div>:!question?<p>{sw?'Ongeza swali ili kuonyesha jaribio.':'Add a question to preview the quiz.'}</p>:(()=>{const text=questionLanguage(question,language);return <>
      <p className="cms-quiz-progress">{sw?'Swali':'Question'} {index+1} / {content.questions.length}</p><progress aria-label="Quiz progress" value={answered?index+1:index} max={content.questions.length}/>
      <h3>{text.text||(sw?'Swali lako':'Your question')}</h3>{text.fallback&&<small>English fallback — translation incomplete</small>}
      <div role="radiogroup" aria-label={sw?'Chaguo za majibu':'Answer choices'}>{text.options.map((o,i)=><button type="button" key={o.id} role="radio" aria-checked={selected===o.id} disabled={answered} onClick={()=>setSelected(o.id)}>{String.fromCharCode(65+i)}. {o.text||'…'}</button>)}</div>
      {!answered?<button type="button" disabled={!selected||!question.correct_option_id} onClick={()=>setAnswered(true)}>{sw?'Angalia jibu':'Check answer'}</button>:<div role="status"><strong>{selected===question.correct_option_id?(sw?'Sahihi':'Correct'):(sw?'Si sahihi':'Not quite')}</strong><p>{text.explanation||'Add an explanation in the builder.'}</p><button type="button" onClick={()=>{if(index===content.questions.length-1)setFinished(true);else{setIndex(index+1);setSelected(null);setAnswered(false);}}}>{index===content.questions.length-1?(sw?'Maliza':'Finish preview'):(sw?'Swali linalofuata':'Next question')}</button></div>}
    </>;})()}
  </section>;
}
