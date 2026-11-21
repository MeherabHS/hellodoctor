'use client';
import { tr } from '@/lib/i18n';
import {PlaySquare} from 'lucide-react';
import {ContentManager} from './ContentManager';
import {VideoEditor} from './VideoEditor';

/** Content → Videos: YouTube videos shown under Videos in the patient app. */
export function VideoManager(){
  return <ContentManager kind="video" title={tr('ui.videos')} noun="video" icon={PlaySquare} Editor={VideoEditor}
    subtitle={tr('ui4.youtubeVideosForThePatientApp')}/>;
}
