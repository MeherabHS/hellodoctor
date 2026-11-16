'use client';
import {ArrowDown,ArrowUp,Plus,Trash2,X} from 'lucide-react';
import {newOption,type QuizQuestion,type QuizLanguage} from '../../lib/quizContent';

/** One multiple-choice question: text, 2–6 options with one correct answer,
 *  and an explanation shown after answering. */
export function QuestionEditor({question,index,total,language,onChange,onRemove,onMove}:{question:QuizQuestion;index:number;total:number;language:QuizLanguage;onChange:(q:QuizQuestion)=>void;onRemove:()=>void;onMove:(offset:number)=>void}){
  const sw=language==='sw';const lang=sw?'Kiswahili':'English';
  const letter=(i:number)=>String.fromCharCode(65+i);
  return <section className="cms-question" aria-label={`Question ${index+1}`}>
    <div className="cms-question-head">
      <strong>Question {index+1}</strong>
      <div style={{display:'flex',gap:4}}>
        <button type="button" className="afya-icon-btn" aria-label="Move up" title="Move up" disabled={index===0} onClick={()=>onMove(-1)}><ArrowUp size={16}/></button>
        <button type="button" className="afya-icon-btn" aria-label="Move down" title="Move down" disabled={index===total-1} onClick={()=>onMove(1)}><ArrowDown size={16}/></button>
        <button type="button" className="afya-icon-btn" aria-label="Remove question" title="Remove question" onClick={onRemove}><Trash2 size={16} color="var(--afya-danger-fg)"/></button>
      </div>
    </div>
    <label className="afya-field-label" htmlFor={`q-${question.id}`}>{lang} question</label>
    <textarea id={`q-${question.id}`} className="afya-input" rows={2} maxLength={2000} value={question[`text_${language}`]} onChange={e=>onChange({...question,[`text_${language}`]:e.target.value})}/>
    <span className="afya-field-label">Answers · select the correct one</span>
    <div className="cms-options">
      {question.options.map((option,i)=><div className={`cms-option ${question.correct_option_id===option.id?'correct':''}`} key={option.id}>
        <input type="radio" name={`answer-${question.id}`} aria-label={`Correct answer ${letter(i)}`} checked={question.correct_option_id===option.id} onChange={()=>onChange({...question,correct_option_id:option.id})}/>
        <span className="cms-option-letter">{letter(i)}</span>
        <input className="afya-input" aria-label={`${lang} option ${letter(i)}`} maxLength={500} value={option[`text_${language}`]} placeholder={`Answer ${letter(i)}`} onChange={e=>onChange({...question,options:question.options.map(o=>o.id===option.id?{...o,[`text_${language}`]:e.target.value}:o)})}/>
        <button type="button" className="afya-icon-btn" aria-label={`Remove option ${letter(i)}`} disabled={question.options.length<=2} onClick={()=>onChange({...question,options:question.options.filter(o=>o.id!==option.id),correct_option_id:question.correct_option_id===option.id?null:question.correct_option_id})}><X size={16}/></button>
      </div>)}
    </div>
    <button type="button" className="afya-link-btn" disabled={question.options.length>=6} onClick={()=>onChange({...question,options:[...question.options,newOption()]})}><Plus size={14} aria-hidden/> Add answer</button>
    <label className="afya-field-label" htmlFor={`x-${question.id}`}>{lang} explanation</label>
    <textarea id={`x-${question.id}`} className="afya-input" rows={2} maxLength={4000} value={question[`explanation_${language}`]} placeholder="Why this answer is correct (shown after answering)" onChange={e=>onChange({...question,[`explanation_${language}`]:e.target.value})}/>
  </section>;
}
