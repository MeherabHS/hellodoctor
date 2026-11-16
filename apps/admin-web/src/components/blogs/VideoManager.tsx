'use client';
import {PlaySquare} from 'lucide-react';
import {ContentManager} from './ContentManager';
import {VideoEditor} from './VideoEditor';

/** Content → Videos: YouTube videos shown under Videos in the patient app. */
export function VideoManager(){
  return <ContentManager kind="video" title="Videos" noun="video" icon={PlaySquare} Editor={VideoEditor}
    subtitle="YouTube videos for the patient app. Tapping one opens YouTube."/>;
}
