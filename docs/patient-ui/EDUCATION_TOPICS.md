# HIV Prevention & Sexual Health — topics, articles, videos, quizzes

Product-owner instruction, 2026-11-13. References in `docs/design/education/`:

- `flow-<topic>.png` — populated flow per topic: hub (Learn) → topic page with "Articles (N)" → article reader.
- `topic-empty-<topic>.jpg` — each topic's own empty state (941 px wide, about 2.29 px per dp).
- `tab-empty-{videos,quizzes,audio}.png` — hub tab empty states with the app header (941 px wide), 2026-11-13. They supersede the earlier small `tab-empty-states.png`.

## How content reaches a topic

| Piece | Where |
|---|---|
| Topic link | `article_categories.topic_key` (one of `HIV_BASICS`, `PREP`, `CONDOMS`, `SRH`, `STIGMA_DISCLOSURE`, `HEALTHY_RELATIONSHIPS`, or none). Admin Web → Articles → Manage categories → "Patient app topic". Migration `20261211000001_education_topics_and_videos` (with down-migration) |
| Topic list | `GET /articles?topic=<KEY>` returns published Admin articles whose category has that key |
| Videos | An Admin article with `video_url` (YouTube only; stored as `https://www.youtube.com/watch?v=<id>`). Same draft → review → publish workflow. `GET /articles?kind=video[&topic=]`. Without `kind`, the feed returns written articles only, as before |
| Quizzes | Two kinds, both listed in the Quizzes tab (standalone first): quizzes attached to an article (Blog Manager → article → MCQs) and **standalone quizzes** (Admin Web → **Quiz Manager**). A standalone quiz is an Admin entry flagged `standalone_quiz` (migration `20261213000001`): title, short introduction, category (sets the topic), optional cover, and questions from the same MCQ builder; `GET /articles?kind=quiz` lists those whose quiz is published. It never appears among articles or videos and cannot carry a video link |
| Approval | One Admin is enough (product owner, 2026-11-13): the author may approve and publish articles, videos, standalone quizzes and quiz revisions, sensitive or not. Migration `20261214000001_single_admin_approval` (reversible) |
| Audio | No content model. The tab shows its empty state |

Nothing in the Patient app is sample content: lists come only from the server.

## Screens

| Screen | File | Matches |
|---|---|---|
| Hub (Learn / Videos / Quizzes / Audio) | `modules/module_screens.dart` (`EducationHubPage`) | Flow left panel. Tabs now show real lists or empty states |
| Topic page | `education/topic_articles_page.dart` | Flow middle panel; topic-empty references |
| Article reader | `blogs/blog_detail_page.dart`, `blogs/article_content.dart` | Flow right panel |
| Videos / Quizzes / Audio tabs | `education/education_tabs.dart` | `tab-empty-states.png` |

## Artwork

- `assets/education/banner_<topic>.png` and `empty_<topic>.png`: cropped from the per-topic empty references (script logic recorded in the commit; sizes measured at 2.29 px/dp). The banner's text is native; only the artwork is an image. Its left edge fades into the card colour so no seam shows.
- `assets/education/tab_empty_{videos,quizzes,audio}.png`: cropped from the full-size tab references, card width, top of card to just above the title. Card colours sampled: Videos #EAF6FE, Quizzes #F1EDFE, Audio #FEECEF.
- Article thumbnails and the reader hero are the article's own cover image, uploaded by an Admin; without a cover, the list falls back to the topic's existing artwork.

## Differences from the reference (deliberate)

| Reference | App | Reason |
|---|---|---|
| Bookmark and Share icons on the reader | Not shown | No bookmark store; articles need sign-in, so a shared link would not open. Product owner chose "hide for now" |
| Infographic blocks inside articles (step cards, "Key areas" icon rows) | Not reproduced | They are article content. Markdown renders headings (green), paragraphs and tick-bullet lists; Admin Markdown disables embedded images and HTML |
| App header (logo, bell with unread count, avatar) | Shown on the hub and on all six topic pages | Shared `AfyaBrandHeader` (`home/brand_header.dart`), also used by Home. The bell opens the inbox; the avatar returns to the shell and opens Profile. Its accessibility label now reads "Notifications" (fixes D-08d) |
| "Browse all articles" button under the topics | Kept | Keeps the route to Doctor-written articles, which belong to no topic |
| Reader: author line | Shown only for Doctor articles | Clinical attribution; Admin articles show none, as in the reference |
| Read time | Words ÷ 200, rounded up, at least 1 min | No read-time field exists; derived from the body |

## Strings

English text is taken from the references. Swahili is machine-quality and needs native review (same status as the rest of the app).

## Tests

- Backend: `admin_articles::test_education_topics_and_videos_feed` (topic filter, video filter, default feed unchanged, YouTube-only, invalid keys 422); domain unit tests for link canonicalisation and topic validation.
- Admin Web: `tests/blog-content.test.ts` (YouTube link validation, topic labels).
- Patient: `test/education_topics_test.dart` (each topic's empty state and artwork, populated rows with EAT date and read time, reader chip and no bookmark/share, Videos/Quizzes/Audio tabs, Swahili at 360 dp and text scale 1.3).
