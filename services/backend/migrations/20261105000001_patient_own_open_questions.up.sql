-- The API role must never read author_patient_id. Expose only owned OPEN IDs
-- using the authenticated transaction context, including guardian profiles.
CREATE FUNCTION patient_owned_open_question_ids() RETURNS TABLE(id UUID)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog,public AS $$
 SELECT q.id FROM public.health_questions q
 WHERE q.status='OPEN'
   AND current_setting('app.current_role',true)='PATIENT'
   AND EXISTS (SELECT 1 FROM public.users u
       WHERE u.id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
         AND u.role='PATIENT' AND u.is_active AND u.deleted_at IS NULL)
   AND public.fn_patient_profile_owned_by_caller(q.author_patient_id,
       NULLIF(current_setting('app.current_user_id',true),'')::uuid)
$$;
REVOKE ALL ON FUNCTION patient_owned_open_question_ids() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION patient_owned_open_question_ids() TO hellodoctor_api;
