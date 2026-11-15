'use client';
import {BookOpen} from 'lucide-react';
import {ArticleEditor} from './ArticleEditor';
import {ContentManager} from './ContentManager';

/** Content → Articles: written articles only (videos and quizzes have their own sections). */
export function BlogManager(){
  return <ContentManager kind="article" title="Articles" noun="article" icon={BookOpen} showTopic Editor={ArticleEditor}
    subtitle="Health articles for the patient app, by topic."/>;
}
