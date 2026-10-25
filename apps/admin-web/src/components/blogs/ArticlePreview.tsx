/* eslint-disable @next/next/no-img-element -- Covers require authenticated uncached requests; local draft files use data URLs. */
import type {ArticleContent,ArticleCategory,AdminArticle} from '../../lib/blogContent';
import {articleLanguage} from '../../lib/blogContent';
import {SafeMarkdown} from './SafeMarkdown';
import {QuizPreview} from './QuizPreview';
import type {QuizContent} from '../../lib/quizContent';

export function ArticlePreview({content,category,language,setLanguage,article,localCover,quiz}:{content:ArticleContent;category?:ArticleCategory;language:'en'|'sw';setLanguage:(v:'en'|'sw')=>void;article:AdminArticle|null;localCover:string;quiz?:QuizContent|null}){
  const text=articleLanguage(content,language);
  const cover=localCover||(article?.cover_image_id?`/api/article-media/${article.id}?v=${article.version}`:'');
  return <section className="cms-preview" aria-label="Patient article preview">
    <div className="cms-preview-label"><strong>Live Patient preview</strong><span>Unsaved preview · never publishes</span></div>
    <div className="cms-language" role="group" aria-label="Preview language">
      <button type="button" aria-pressed={language==='en'} onClick={()=>setLanguage('en')}>English</button>
      <button type="button" aria-pressed={language==='sw'} onClick={()=>setLanguage('sw')}>Kiswahili</button>
    </div>
    <div className="cms-phone"><div className="cms-phone-bar">9:41 <span>● ▰</span></div><div className="cms-phone-content">
      <div className="cms-phone-nav">← &nbsp; Health articles</div>
      {cover&&<img className="cms-cover" src={cover} alt="Article cover"/>}
      {category&&<small className="cms-category">{language==='sw'?category.name_sw:category.name_en}</small>}
      <h1>{text.title||'Your article title'}</h1>
      <p className="cms-byline">AfyaMkononi Editorial<br/>{article?.published_at?new Date(article.published_at).toLocaleDateString('en-KE',{timeZone:'Africa/Nairobi'}):'Draft · publication date appears after publishing'}</p>
      {text.fallback&&<small>English fallback — Kiswahili translation unavailable</small>}
      <div className="cms-markdown"><SafeMarkdown body={text.body||'Start writing to preview your article here.'}/></div>
      {quiz&&<QuizPreview key={JSON.stringify(quiz)} content={quiz} language={language}/>}
    </div></div>
    <p className="cms-hint">Approximates the Patient article screen. Flutter typography and layout may differ.</p>
  </section>;
}
