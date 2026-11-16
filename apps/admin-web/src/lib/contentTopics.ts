import type {ArticleCategory,TopicKey} from './blogContent';
import {educationTopics} from './blogContent';

/** What the Admin picks first for an article: one of the six Patient app
 *  topics, or "Other" with a name they type. Each choice is stored as an
 *  article category (created on first use), so the backend contract and the
 *  Patient app are unchanged: a topic category lists under its topic, an
 *  "Other" category appears in the general article list. */
export type TopicChoice={topic:TopicKey|'OTHER'|'';other:string};

export const OTHER='OTHER' as const;

/** Kiswahili names, as the Patient app shows them (app_sw.arb eduTopic*). */
const TOPIC_SW:Record<TopicKey,string>={
  HIV_BASICS:'Msingi wa VVU',PREP:'PrEP',CONDOMS:'Kondomu',SRH:'Afya ya Uzazi na Ujinsia',
  STIGMA_DISCLOSURE:'Unyanyapaa na Kujiweka Wazi',HEALTHY_RELATIONSHIPS:'Mahusiano Yenye Afya',
};

export const topicOptions=[...educationTopics.map(t=>({value:t.key as TopicKey|'OTHER',label:t.label})),{value:OTHER as TopicKey|'OTHER',label:'Other'}];

/** Videos and quizzes are listed on their own tabs in the Patient app, not
 *  under a topic; they still need a category, so they share one each. */
export const VIDEO_CATEGORY={name_en:'Videos',name_sw:'Video'};
export const QUIZ_CATEGORY={name_en:'Quizzes',name_sw:'Maswali ya Kujipima'};

const same=(a:string,b:string)=>a.trim().toLowerCase()===b.trim().toLowerCase();

export function choiceFor(category:ArticleCategory|undefined):TopicChoice{
  if(!category)return {topic:'',other:''};
  return category.topic_key?{topic:category.topic_key,other:''}:{topic:OTHER,other:category.name_en};
}

export function choiceErrors(choice:TopicChoice):string[]{
  if(!choice.topic)return ['Choose a topic.'];
  if(choice.topic===OTHER){
    const name=choice.other.trim();
    if(Array.from(name).length<2||Array.from(name).length>80)return ['Write the topic name (2–80 characters).'];
  }
  return [];
}

type SaveCategory=(category:Omit<ArticleCategory,'id'>,id?:string)=>Promise<ArticleCategory>;

/** The category for a choice: an existing one when it matches, otherwise a
 *  new one. Returns its id and whether the list changed. */
export async function categoryFor(choice:TopicChoice,categories:ArticleCategory[],save:SaveCategory):Promise<{id:string;created:boolean}>{
  const errors=choiceErrors(choice);if(errors.length)throw new Error(errors[0]);
  if(choice.topic===OTHER)return named({name_en:choice.other.trim(),name_sw:choice.other.trim()},categories,save,null);
  const key=choice.topic as TopicKey;
  const linked=categories.find(c=>c.topic_key===key);
  if(linked)return {id:linked.id,created:false};
  const label=educationTopics.find(t=>t.key===key)!.label;
  return named({name_en:label,name_sw:TOPIC_SW[key]},categories,save,key);
}

/** A category by name; one with the same name but no topic is linked to
 *  [topic] rather than duplicated (names are unique). */
export async function named(names:{name_en:string;name_sw:string},categories:ArticleCategory[],save:SaveCategory,topic:TopicKey|null=null):Promise<{id:string;created:boolean}>{
  const existing=categories.find(c=>same(c.name_en,names.name_en)||same(c.name_sw,names.name_sw));
  if(existing){
    if(topic&&!existing.topic_key){await save({name_en:existing.name_en,name_sw:existing.name_sw,sensitive:existing.sensitive,topic_key:topic},existing.id);return {id:existing.id,created:true};}
    return {id:existing.id,created:false};
  }
  const created=await save({...names,sensitive:true,topic_key:topic});
  return {id:created.id,created:true};
}

/** The 11-character id of a YouTube link, or null. */
export function youTubeId(link:string):string|null{
  const m=link.trim().match(/^https?:\/\/(?:(?:www\.|m\.)?youtube\.com\/(?:watch\?(?:[^#]*&)?v=|shorts\/|embed\/|live\/)|youtu\.be\/)([A-Za-z0-9_-]{11})(?:[?&#/].*)?$/i);
  return m?m[1]:null;
}
