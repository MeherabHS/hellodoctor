-- Narrow projection/evaluation bridge: never grant Patient SELECT on answer-key rows.
CREATE FUNCTION lock_patient_article_quiz(article UUID) RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp SET row_security=off AS $$
DECLARE actor UUID; parent_version INTEGER; parent_approved INTEGER; parent_status TEXT; publication public.article_quiz_publications%ROWTYPE; revision public.article_quiz_revisions%ROWTYPE;
BEGIN
 actor:=NULLIF(current_setting('app.current_user_id',true),'')::uuid;
 IF current_setting('app.current_role',true) IS DISTINCT FROM 'PATIENT'
 OR NOT EXISTS(SELECT 1 FROM public.users WHERE id=actor AND role='PATIENT' AND is_active) THEN
  RAISE EXCEPTION 'Patient required' USING ERRCODE='42501';
 END IF;
 -- Same parent-first order as Admin quiz publication. Locks live until API commit.
 SELECT content_version,approved_version,status INTO parent_version,parent_approved,parent_status FROM public.admin_articles WHERE id=article FOR SHARE;
 IF parent_status IS DISTINCT FROM 'PUBLISHED' OR parent_approved IS DISTINCT FROM parent_version THEN RETURN NULL; END IF;
 SELECT * INTO publication FROM public.article_quiz_publications WHERE article_id=article FOR SHARE;
 IF NOT FOUND OR NOT publication.included OR publication.article_content_version<>parent_version THEN RETURN NULL; END IF;
 SELECT * INTO revision FROM public.article_quiz_revisions WHERE id=publication.revision_id AND article_id=article FOR SHARE;
 IF NOT FOUND OR revision.status<>'PUBLISHED' OR NOT revision.include_quiz OR revision.approved_version IS DISTINCT FROM revision.content_version OR revision.article_content_version<>parent_version THEN RETURN NULL; END IF;
 RETURN revision.id;
END $$;
REVOKE ALL ON FUNCTION lock_patient_article_quiz(UUID) FROM PUBLIC;

CREATE FUNCTION read_patient_article_quiz(article UUID) RETURNS JSONB
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp SET row_security=off AS $$
DECLARE revision UUID; questions JSONB;
BEGIN
 revision:=public.lock_patient_article_quiz(article);
 IF revision IS NULL THEN RETURN jsonb_build_object('state','UNAVAILABLE'); END IF;
 SELECT jsonb_agg(jsonb_build_object('id',q.id,'text_en',q.text_en,'text_sw',CASE WHEN translated THEN q.text_sw ELSE '' END,'options',
  (SELECT jsonb_agg(jsonb_build_object('id',o.id,'text_en',o.text_en,'text_sw',CASE WHEN translated THEN o.text_sw ELSE '' END) ORDER BY o.position) FROM public.article_quiz_options o WHERE o.revision_id=revision AND o.question_id=q.id)) ORDER BY q.position)
 INTO questions FROM (
  SELECT question.*,btrim(text_sw)<>'' AND btrim(explanation_sw)<>'' AND NOT EXISTS(SELECT 1 FROM public.article_quiz_options choice WHERE choice.revision_id=revision AND choice.question_id=question.id AND btrim(choice.text_sw)='') AS translated
  FROM public.article_quiz_questions question WHERE revision_id=revision
 ) q;
 IF questions IS NULL OR jsonb_array_length(questions) NOT BETWEEN 1 AND 50 THEN RETURN jsonb_build_object('state','UNAVAILABLE'); END IF;
 RETURN jsonb_build_object('schema_version',1,'article_id',article,'revision_id',revision,'included',true,'question_count',jsonb_array_length(questions),'questions',questions);
END $$;
REVOKE ALL ON FUNCTION read_patient_article_quiz(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION read_patient_article_quiz(UUID) TO hellodoctor_api;

CREATE FUNCTION check_patient_article_quiz(article UUID, expected_revision UUID, question UUID, selected_option UUID) RETURNS JSONB
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp SET row_security=off AS $$
DECLARE revision UUID; item public.article_quiz_questions%ROWTYPE; translated BOOLEAN;
BEGIN
 revision:=public.lock_patient_article_quiz(article);
 IF revision IS NULL THEN RETURN jsonb_build_object('state','UNAVAILABLE'); END IF;
 IF revision<>expected_revision THEN RETURN jsonb_build_object('state','SUPERSEDED'); END IF;
 SELECT * INTO item FROM public.article_quiz_questions WHERE revision_id=revision AND id=question;
 IF NOT FOUND OR NOT EXISTS(SELECT 1 FROM public.article_quiz_options WHERE revision_id=revision AND question_id=question AND id=selected_option) THEN RETURN jsonb_build_object('state','INVALID_CHOICE'); END IF;
 translated:=btrim(item.text_sw)<>'' AND btrim(item.explanation_sw)<>'' AND NOT EXISTS(SELECT 1 FROM public.article_quiz_options WHERE revision_id=revision AND question_id=question AND btrim(text_sw)='');
 RETURN jsonb_build_object('schema_version',1,'article_id',article,'revision_id',revision,'question_id',question,'correct',item.correct_option_id=selected_option,'correct_option_id',item.correct_option_id,'explanation_en',item.explanation_en,'explanation_sw',CASE WHEN translated THEN item.explanation_sw ELSE '' END);
END $$;
REVOKE ALL ON FUNCTION check_patient_article_quiz(UUID,UUID,UUID,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION check_patient_article_quiz(UUID,UUID,UUID,UUID) TO hellodoctor_api;
