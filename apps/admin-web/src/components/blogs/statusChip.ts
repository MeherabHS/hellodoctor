import type {ArticleStatus} from '../../lib/blogContent';
import type {ChipTone} from '../ui/afya';

/** Status chip colour and label for articles, videos and quizzes. */
export const STATUS_CHIP:Record<ArticleStatus,{tone:ChipTone;label:string}>={
  DRAFT:{tone:'warn',label:'Draft'},PENDING_REVIEW:{tone:'warn',label:'Draft'},APPROVED:{tone:'warn',label:'Draft'},
  SCHEDULED:{tone:'info',label:'Scheduled'},PUBLISHED:{tone:'success',label:'Published'},ARCHIVED:{tone:'neutral',label:'Archived'},
};
