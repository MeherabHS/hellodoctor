'use client';
import { tr } from '@/lib/i18n';
import {ListChecks} from 'lucide-react';
import {ContentManager} from './ContentManager';
import {QuizEditor} from './QuizEditor';

/** Content → Quizzes: multiple-choice quizzes, listed under HIV Prevention &
 *  Sexual Health → Quizzes in the patient app. */
export function QuizManager(){
  return <ContentManager kind="quiz" title={tr('ui.quizzes')} noun="quiz" icon={ListChecks} Editor={QuizEditor}
    subtitle={tr('ui4.multipleChoiceQuizzesForThePatient')}/>;
}
