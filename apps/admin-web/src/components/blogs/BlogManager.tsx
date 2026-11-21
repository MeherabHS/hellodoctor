'use client';
import { tr } from '@/lib/i18n';
import {BookOpen} from 'lucide-react';
import {ArticleEditor} from './ArticleEditor';
import {ContentManager} from './ContentManager';

/** Content → Articles: written articles only (videos and quizzes have their own sections). */
export function BlogManager(){
  return <ContentManager kind="article" title={tr('ui.articles')} noun="article" icon={BookOpen} showTopic Editor={ArticleEditor}
    subtitle={tr('ui4.healthArticlesForThePatientApp')}/>;
}
