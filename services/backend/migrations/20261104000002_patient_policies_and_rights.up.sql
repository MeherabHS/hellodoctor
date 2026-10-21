-- Patient documents are selected explicitly by the Patient registration service.
-- required=false keeps them out of the Doctor registration required set.
INSERT INTO legal_documents(document_type,version,title,required,is_current,body) VALUES
('PATIENT_TERMS','1.0','Patient Terms of Service',false,true,$doc$Patient Terms of Service (v1.0)

HelloDoctor provides account, booking, communication and consultation tools. Your treating practitioner is responsible for clinical assessment and any required treatment consent. Remote consultations have limitations; an examination, referral or in-person care may be necessary. For emergencies seek local emergency services immediately; do not wait for this app.

Create an account only with accurate contact and profile details. Self-registration is for adults aged 18 or older. Use family profiles for children or dependants only where you have lawful authority. Keep passwords and verification codes private. Do not impersonate another person, upload unlawful material, abuse staff, or attempt unauthorized access.

Review the fee, appointment details and applicable cancellation/refund information before confirming each booking. These terms do not introduce additional charges or waive any statutory refund, consumer, patient or privacy rights. Service availability depends on connectivity and practitioner availability; no clinical outcome is guaranteed.

We may restrict misuse where necessary and proportionate, subject to applicable law. Material changes require a new published version. Use Legal & your data in your profile for privacy requests; use the clinical grievance process for a care complaint. You may also complain to the appropriate regulator, including the ODPC for data protection matters. Kenyan law applies. Nothing here excludes liability that cannot lawfully be excluded or removes your right to seek a legal remedy.$doc$),
('PATIENT_PRIVACY_NOTICE','1.0','Patient Privacy Policy',false,true,$doc$Patient Privacy Policy (v1.0)

HelloDoctor processes the full name, birth date/age, gender, phone and email you provide, your account credentials in protected form, and the appointment, health, prescription, payment reference, support and security information you use in the service. We use this information for account verification, arranging and documenting care, communication, payment administration, support, fraud prevention and legal obligations. Required registration fields support identification, age safeguards and secure contact; without them an account cannot be created.

Access is limited according to role and care relationship. Information necessary for your care may be available to your treating practitioner. Contracted infrastructure, communications, payment and support providers process only what is needed for their service. Email verification uses Azure Communication Services when configured and sends your email address and an authentication message, not your clinical record. Development mobile testing uses a local backend and an explicitly labelled development code instead of email delivery. Do not enter real medical information into that test environment.

Processing must have an appropriate lawful basis under Kenyan law. Consent is used where required; a contract, legal obligation or other permitted health-care basis may apply to other processing. Acceptance of this notice is acknowledgement, not blanket permission for unrelated purposes. Account creation does not authorize marketing, research, sale of health data or unrestricted disclosure. Those purposes require their own legal basis and any necessary separate consent and approvals.

We retain information only for its lawful purpose and applicable medical, financial, security and legal retention requirements. A deletion request is assessed against those requirements; retained records and reasons must be explained to you. Access controls, secure credential storage and audit records support protection, but no service can promise zero risk. Any overseas processing requires the safeguards and permissions applicable to sensitive health information; this notice alone does not authorize an overseas transfer.

You can request access/a copy, correction, deletion, restriction, objection, portability where applicable, or withdrawal of consent through Legal & your data in your profile. We verify identity proportionately and record a request reference. Withdrawal does not invalidate earlier lawful processing. You may complain to the Office of the Data Protection Commissioner at https://www.odpc.go.ke/file-a-complaint/ without surrendering other remedies.

This is the development/pilot policy. Before public use, the operator must publish its registered legal identity, direct privacy/DPO contact, actual processor list and hosting locations, specific retention schedule and approved transfer arrangements. Production onboarding must remain disabled until these details and operational request handling have been reviewed.$doc$),
('PATIENT_DATA_RIGHTS','1.0','Data Access, Deletion & Claims',false,true,$doc$Data Access, Deletion & Claims (v1.0)

Your information remains subject to your statutory rights. In Profile > Legal & your data you can submit and track a request for a copy/access, correction, deletion/account closure, restriction or objection, portability where applicable, consent withdrawal, or a privacy complaint/claim. Select the request type and describe the scope without adding medical details, passwords or OTPs. You receive a request reference and submission time. Use your own account; requests for another person require verified lawful authority.

Submission records a request for review; it does not immediately erase a clinical record or close your account. The operator must assess the applicable legal deadline, verify identity where necessary, provide a response and reasons for any lawful refusal or retained records, and arrange secure delivery of any copy. Medical records, payments, disputes and legal holds may require retention. Consent withdrawal applies to processing relying on consent and cannot undo earlier lawful processing. A request can be made without agreeing to optional marketing or research.

If the handling does not resolve your concern you can complain to the Office of the Data Protection Commissioner: https://www.odpc.go.ke/file-a-complaint/. You retain your right to regulatory or judicial remedies. No terms or consent form provides the operator with legal immunity or waives those rights.$doc$);

CREATE TABLE patient_data_requests (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
 user_id UUID NOT NULL REFERENCES users(id),
 request_type TEXT NOT NULL CHECK(request_type IN ('ACCESS','CORRECTION','DELETION','WITHDRAWAL','RESTRICTION','PORTABILITY','COMPLAINT')),
 details TEXT NOT NULL CHECK(char_length(details) BETWEEN 1 AND 2000),
 status TEXT NOT NULL DEFAULT 'SUBMITTED' CHECK(status IN ('SUBMITTED','IN_REVIEW','COMPLETED','DECLINED')),
 response TEXT CHECK(char_length(response) <= 4000),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX patient_data_requests_owner_idx ON patient_data_requests(user_id,created_at DESC);
ALTER TABLE patient_data_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE patient_data_requests FORCE ROW LEVEL SECURITY;
GRANT SELECT,INSERT ON patient_data_requests TO hellodoctor_api;
GRANT UPDATE(status,response,updated_at) ON patient_data_requests TO hellodoctor_api;
CREATE POLICY patient_data_requests_read ON patient_data_requests FOR SELECT TO hellodoctor_api USING (
 (current_setting('app.current_role',true)='PATIENT' AND user_id=NULLIF(current_setting('app.current_user_id',true),'')::UUID)
 OR current_setting('app.current_role',true) IN ('COMPLIANCE','PLATFORM_ADMIN')
);
CREATE POLICY patient_data_requests_insert ON patient_data_requests FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true)='PATIENT' AND user_id=NULLIF(current_setting('app.current_user_id',true),'')::UUID
 AND status='SUBMITTED' AND response IS NULL
);
CREATE POLICY patient_data_requests_review ON patient_data_requests FOR UPDATE TO hellodoctor_api
 USING (current_setting('app.current_role',true) IN ('COMPLIANCE','PLATFORM_ADMIN'))
 WITH CHECK (current_setting('app.current_role',true) IN ('COMPLIANCE','PLATFORM_ADMIN'));
