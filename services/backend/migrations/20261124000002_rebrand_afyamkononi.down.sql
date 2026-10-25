UPDATE legal_documents
   SET is_current = false
 WHERE version LIKE '%-afyamkononi';

UPDATE legal_documents
   SET is_current = true
 WHERE version NOT LIKE '%-afyamkononi'
   AND document_type IN (
       SELECT document_type FROM legal_documents WHERE version LIKE '%-afyamkononi'
   );

DELETE FROM legal_documents WHERE version LIKE '%-afyamkononi';

UPDATE notification_templates
   SET title_template = replace(title_template, 'AfyaMkononi', 'HelloDoctor'),
       body_template = replace(body_template, 'AfyaMkononi', 'HelloDoctor')
 WHERE template_key IN ('registration_verified', 'registration_rejected', 'professional_verification_revoked')
   AND (title_template LIKE '%AfyaMkononi%' OR body_template LIKE '%AfyaMkononi%');
