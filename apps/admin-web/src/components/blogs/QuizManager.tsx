'use client';
import {ListChecks} from 'lucide-react';
import {ContentManager} from './ContentManager';
import {QuizEditor} from './QuizEditor';

/** Content → Quizzes: multiple-choice quizzes, listed under HIV Prevention &
 *  Sexual Health → Quizzes in the patient app. */
export function QuizManager(){
  return <ContentManager kind="quiz" title="Quizzes" noun="quiz" icon={ListChecks} Editor={QuizEditor}
    subtitle="Multiple-choice quizzes for the patient app."/>;
}
