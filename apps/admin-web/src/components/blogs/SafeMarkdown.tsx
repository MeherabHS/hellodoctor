import Markdown from 'react-markdown';
import {safeArticleLink} from '../../lib/blogContent';

export function SafeMarkdown({body}:{body:string}){
  return <Markdown skipHtml allowedElements={['h1','h2','h3','h4','h5','h6','p','strong','em','ul','ol','li','a','blockquote','br','hr','code','pre']}
    urlTransform={safeArticleLink} components={{a:({href,children})=>href?<a href={href} target="_blank" rel="noopener noreferrer">{children}</a>:<span>{children}</span>}}>{body}</Markdown>;
}
