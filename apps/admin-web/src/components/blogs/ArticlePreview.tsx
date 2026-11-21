import { tr } from '@/lib/i18n';
/* eslint-disable @next/next/no-img-element -- Covers require authenticated uncached requests; local draft files use data URLs. */
import type {ArticleContent,ArticleCategory,AdminArticle} from '../../lib/blogContent';
import {articleLanguage} from '../../lib/blogContent';
import {SafeMarkdown} from './SafeMarkdown';
import {QuizPreview} from './QuizPreview';
import type {QuizContent} from '../../lib/quizContent';

export function ArticlePreview({content,category,language,setLanguage,article,localCover,quiz}:{content:ArticleContent;category?:ArticleCategory;language:'en'|'sw';setLanguage:(v:'en'|'sw')=>void;article:AdminArticle|null;localCover:string;quiz?:QuizContent|null}){
  const text=articleLanguage(content,language);
  const cover=localCover||(article?.cover_image_id?`/api/article-media/${article.id}?v=${article.version}`:'');
  return <section className="cms-preview" aria-label={tr('ui.patientArticlePreview')}>
    <div className="cms-preview-label"><strong>{tr('ui.patientAppPreview')}</strong><span>{tr('ui.showsUnsavedChangesNeverPublishes')}</span></div>
    <div className="afya-lang-switch" role="group" aria-label={tr('ui.previewLanguage')}>
      <button type="button" aria-pressed={language==='en'} onClick={()=>setLanguage('en')}>{tr('ui.english')}</button>
      <button type="button" aria-pressed={language==='sw'} onClick={()=>setLanguage('sw')}>{tr('ui.kiswahili')}</button>
    </div>
    <div className="cms-phone"><div className="cms-phone-bar">9:41 <span>● ▰</span></div><div className="cms-phone-content">
      <div className="cms-phone-nav">{tr('ui.healthArticles')}</div>
      {cover&&<img className="cms-cover" src={cover} alt={tr('ui.articleCover')}/>}
      {category&&<small className="cms-category">{language==='sw'?category.name_sw:category.name_en}</small>}
      <h1>{text.title||tr('ui2.yourArticleTitle')}</h1>
      <p className="cms-byline">{tr('ui.afyamkononiEditorial')}<br/>{article?.published_at?new Date(article.published_at).toLocaleDateString('en-KE',{timeZone:'Africa/Nairobi'}):tr('ui2.draftPublicationDateAppearsAfterPublishi')}</p>
      {text.fallback&&<small>{tr('ui.englishFallbackKiswahiliTranslationUnava')}</small>}
      <div className="cms-markdown"><SafeMarkdown body={text.body||tr('ui2.startWritingToPreviewYourArticle')}/></div>
      {quiz&&<QuizPreview key={JSON.stringify(quiz)} content={quiz} language={language}/>}
    </div></div>
    <p className="cms-hint">{tr('ui.approximatesThePatientArticleScreenFlutt')}</p>
  </section>;
}
