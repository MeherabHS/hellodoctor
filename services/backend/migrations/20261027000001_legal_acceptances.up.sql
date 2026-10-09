-- Doctor onboarding legal/consent gate. Registration (auth_handlers::doctor_register)
-- requires every CURRENT required document to be accepted atomically with the
-- account; see docs/ADR/ADR-011-legal-consent-ledger.md.
--
-- legal_documents is a server-owned registry: the single source of truth for
-- what text and version is currently required. document_hash is recomputed
-- by Postgres itself (via a trigger below) from body, so it can never drift
-- from the text actually shown, and no client or application code ever
-- supplies it.
CREATE TYPE legal_document_type_enum AS ENUM (
    'DOCTOR_TERMS',
    'PRIVACY_NOTICE',
    'PROFESSIONAL_VERIFICATION_DECLARATION',
    'TELEMEDICINE_PRACTITIONER_POLICY',
    'MARKETING_COMMUNICATIONS'
);

CREATE TABLE legal_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type legal_document_type_enum NOT NULL,
    version TEXT NOT NULL CHECK (char_length(version) BETWEEN 1 AND 20),
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 200),
    body TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 20000),
    document_hash TEXT NOT NULL DEFAULT '',
    required BOOLEAN NOT NULL,
    is_current BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_type, version)
);
-- At most one current version per document type.
CREATE UNIQUE INDEX legal_documents_one_current ON legal_documents(document_type) WHERE is_current;

-- Postgres generated columns must be STRICTLY IMMUTABLE, and convert_to()
-- is only STABLE, so the hash can't be declared `GENERATED ALWAYS AS (...)`.
-- A BEFORE trigger gives the identical guarantee instead: document_hash is
-- always recomputed from body by Postgres itself, on every insert and
-- every body change, and no application code ever supplies it.
CREATE FUNCTION compute_legal_document_hash() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.document_hash := encode(sha256(convert_to(NEW.body, 'UTF8')), 'hex');
    RETURN NEW;
END $$;
CREATE TRIGGER legal_document_hash_guard BEFORE INSERT OR UPDATE OF body ON legal_documents
    FOR EACH ROW EXECUTE FUNCTION compute_legal_document_hash();

-- Reference data: no per-row ownership, so no RLS — a plain role GRANT
-- governs who can read it. hellodoctor_auth_worker is the only reader: the
-- public GET /legal/documents endpoint and the registration-time
-- required-set check both run pre-session, through state.auth_db, the same
-- way doctor_register already looks up duplicate license/phone/email there.
GRANT SELECT ON legal_documents TO hellodoctor_auth_worker;

-- legal_acceptances is an immutable, append-only evidence ledger: canonical
-- users.id is the accepting identity, document_hash is always the hash
-- Postgres computed for that exact version (never client-supplied), and
-- nothing can UPDATE or DELETE a row once written.
CREATE TABLE legal_acceptances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    document_type legal_document_type_enum NOT NULL,
    document_version TEXT NOT NULL,
    document_hash TEXT NOT NULL,
    accepted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    request_id TEXT,
    session_id UUID,
    ip_hash TEXT,
    app_version TEXT,
    platform TEXT,
    locale TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (document_type, document_version) REFERENCES legal_documents(document_type, version)
);
CREATE INDEX legal_acceptances_user ON legal_acceptances(user_id, document_type, accepted_at DESC);

-- Belt-and-suspenders on top of the handler-level required-set check: an
-- acceptance can only ever reference the document version that is current
-- *at the moment of acceptance*, and the stored hash is always recomputed
-- from that row, never trusted from the inserting statement.
CREATE FUNCTION guard_legal_acceptance_current() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE doc RECORD;
BEGIN
    SELECT is_current, document_hash INTO doc FROM legal_documents
        WHERE document_type = NEW.document_type AND version = NEW.document_version;
    IF NOT FOUND OR NOT doc.is_current THEN
        RAISE EXCEPTION 'Only the current version of a legal document may be accepted' USING ERRCODE = '23514';
    END IF;
    NEW.document_hash := doc.document_hash;
    RETURN NEW;
END $$;
CREATE TRIGGER legal_acceptance_current_guard BEFORE INSERT ON legal_acceptances
    FOR EACH ROW EXECUTE FUNCTION guard_legal_acceptance_current();

ALTER TABLE legal_acceptances ENABLE ROW LEVEL SECURITY;
ALTER TABLE legal_acceptances FORCE ROW LEVEL SECURITY;
-- Doctor reads their own acceptance history; Compliance/PlatformAdmin/
-- ClinicalAdmin read any doctor's as evidence (see abac::Action::ViewLegalAcceptances).
CREATE POLICY legal_acceptances_owner_or_compliance_read ON legal_acceptances FOR SELECT TO hellodoctor_api USING (
    user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid OR
    current_setting('app.current_role', true) IN ('COMPLIANCE', 'PLATFORM_ADMIN', 'CLINICAL_ADMIN'));
-- Written only inside the registration transaction (hellodoctor_auth_worker),
-- for the authenticating subject itself.
CREATE POLICY legal_acceptances_auth_insert ON legal_acceptances FOR INSERT TO hellodoctor_auth_worker WITH CHECK (
    user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
-- No UPDATE or DELETE policy exists for any role, and no role is granted
-- those privileges below — immutability holds even against direct SQL
-- through either ordinary capability role, not just through handler code.
GRANT SELECT ON legal_acceptances TO hellodoctor_api;
GRANT SELECT, INSERT ON legal_acceptances TO hellodoctor_auth_worker;

-- Seed the current v1.0 documents. Placeholder copy grounded in Kenya ODPC
-- guidance (cross-border/Azure processing disclosure, KMPDC verification
-- authorization, telemedicine confidentiality) — NOT lawyer-reviewed; must be
-- replaced with legal/compliance-approved text before production launch.
INSERT INTO legal_documents (document_type, version, title, required, is_current, body) VALUES
('DOCTOR_TERMS', '1.0', 'HelloDoctor Doctor Terms of Service', true, true, $doc$HelloDoctor Doctor Terms of Service (v1.0)

By registering as a Doctor on HelloDoctor you agree to the following terms.

1. Eligibility. You hold a current, valid Kenya Medical Practitioners and Dentists Council (KMPDC) registration in good standing, and the professional details you submit are accurate and complete.

2. Account security. You are responsible for keeping your account credentials and multi-factor authentication confidential, and for all activity under your account. You must never share your login with anyone.

3. Clinical conduct. You remain fully and solely responsible for the clinical judgment, advice, and documentation you provide during every consultation, regardless of outcome (COMPLETED_WITH_RX, COMPLETED_NO_RX, REFERRED, or ESCALATED).

4. Earnings. Doctor earnings are calculated as Gross Earnings minus a 20% platform charge, producing your Final Net Take-Home. This calculation, including the 20% platform charge, is shown on every earnings view.

5. No vanity ratings. HelloDoctor does not use star ratings, public reviews, or popularity rankings. Patient dissatisfaction is handled exclusively through the Clinical Grievance Redressal System.

6. Prohibited use. You must not use the platform to solicit patients for off-platform treatment, submit false registration or verification details, or act outside the scope of your professional authorization.

7. Suspension and termination. HelloDoctor may suspend or terminate your account for breach of these terms, a failed or revoked professional verification, or conduct that endangers patient safety.$doc$),

('PRIVACY_NOTICE', '1.0', 'HelloDoctor Privacy & Data Processing Notice', true, true, $doc$HelloDoctor Privacy & Data Processing Notice (v1.0)

This notice explains how HelloDoctor collects and processes your data as a registered Doctor.

1. Data we collect. Identity and contact details, KMPDC registration number and qualifications, professional experience and hospital affiliation, account security data, and technical/usage data needed to operate the platform.

2. Why we process it. To verify your professional credentials, operate your account and clinical workflow, process consultation payments and the 20% platform charge settlement, maintain platform safety and security, and meet legal and regulatory obligations.

3. Legal bases. Depending on the activity, processing relies on performance of our contract with you, a legal obligation, our legitimate interest in platform safety and integrity, or your consent (for example, optional marketing communications).

4. Cross-border processing. Some HelloDoctor infrastructure, including cloud hosting, is operated outside Kenya (including Microsoft Azure regions outside Kenya). Where this occurs, HelloDoctor applies contractual and technical safeguards to protect your data to a standard consistent with Kenya's Data Protection Act, 2019.

5. Retention. Professional and account records are retained for as long as your account is active and for the period required afterward to meet legal, regulatory, and dispute-resolution obligations.

6. Your rights. Under Kenya's Data Protection Act, 2019, you have the right to access, correct, and in some cases object to or restrict processing of your data, and to lodge a complaint with the Office of the Data Protection Commissioner (ODPC).

7. Contact. Questions about this notice, or requests to exercise your rights, can be directed to HelloDoctor's Data Protection Officer through the contact details published in the app.$doc$),

('PROFESSIONAL_VERIFICATION_DECLARATION', '1.0', 'Professional Verification Declaration', true, true, $doc$Professional Verification Declaration (v1.0)

I confirm that the professional registration number, qualifications, and supporting documents I have submitted are true, complete, and current.

I authorize HelloDoctor to verify these details directly with the Kenya Medical Practitioners and Dentists Council (KMPDC) and, where relevant, with any hospital or institution I have named as my current affiliation.

I understand that submitting false or misleading professional information is grounds for immediate suspension of my account and may be reported to KMPDC and any other relevant professional authority.

I confirm that I am not currently subject to any suspension, disciplinary finding, or restriction that would prevent me from lawfully practicing medicine in Kenya.

I understand that this declaration does not itself verify my account — professional verification remains PENDING until HelloDoctor's Clinical Admin team independently confirms my registration.$doc$),

('TELEMEDICINE_PRACTITIONER_POLICY', '1.0', 'Telemedicine & Confidentiality Declaration', true, true, $doc$Telemedicine & Confidentiality Declaration (v1.0)

I agree to provide consultations on HelloDoctor only within the scope of my professional authorization and clinical competence, and to refer or escalate any patient whose needs exceed what a teleconsultation can safely address.

I agree to protect patient confidentiality and health information at all times, in line with my professional and legal obligations, and to access patient records only as needed to provide care.

I agree to use only my own individual HelloDoctor account and enabled security features (including multi-factor authentication), and never to share my credentials or allow another person to consult under my identity.

I agree to follow HelloDoctor's clinical documentation requirements, including recording an accurate consultation outcome (COMPLETED_WITH_RX, COMPLETED_NO_RX, REFERRED, or ESCALATED) for every consultation.

I understand that patient dissatisfaction with my care is handled exclusively through HelloDoctor's structured Clinical Grievance Redressal System, not through public ratings or reviews.$doc$),

('MARKETING_COMMUNICATIONS', '1.0', 'Product Updates & Communications', false, true, $doc$Product Updates & Communications (v1.0)

I agree to receive occasional non-essential communications from HelloDoctor, such as product updates, new feature announcements, and platform news, by email, SMS, or push notification.

This is entirely optional and independent of the agreements required to register. You can withdraw this consent at any time from your account settings without affecting your ability to use HelloDoctor.$doc$);
