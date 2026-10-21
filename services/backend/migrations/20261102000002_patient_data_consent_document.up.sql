-- This migration must follow the enum-only migration. PostgreSQL requires a
-- newly added enum value to be committed before it is used in DML.
INSERT INTO legal_documents
    (document_type, version, title, required, is_current, body)
VALUES
('PATIENT_DATA_SHARING_CONSENT', '1.0', 'HelloDoctor Patient Data & Health Information Consent', false, true, $doc$HelloDoctor Patient Data & Health Information Consent (v1.0)

Please read this before creating your HelloDoctor account.

1. What we collect. We collect the contact, identity, profile, appointment, consultation, prescription, intake-document, grievance, device and security information needed to provide and protect HelloDoctor services.

2. Why we use it. We use this information to verify your account, arrange and document care, enable communication with your care team, process payments and support requests, maintain safety and security, meet legal obligations, and keep accurate health records.

3. Who may receive it. We share only the information needed for the stated purpose with the Doctor and care team involved in your consultation, service providers that process data for HelloDoctor under confidentiality and security obligations, and public authorities or other recipients where Kenyan law requires or permits disclosure. We do not sell your health information.

4. Data sharing. A request for identifiable health information by another person or organization requires an appropriate lawful basis and, where required by Kenyan digital-health rules, a data-sharing agreement and your consent. Research or public-health use must follow the required approvals and safeguards; where possible, information is de-identified or aggregated.

5. Your choices and rights. You may request access to, correction of, or information about the use of your personal data, object to processing where the law allows, and withdraw consent for processing that relies on consent. Withdrawal does not undo processing already carried out lawfully and does not override care, legal, safety, payment, or record-retention obligations. You may complain to the Office of the Data Protection Commissioner.

6. Children and dependants. A parent or lawful guardian must act for a child or dependant and must use the family-profile controls only for people they are authorized to represent.

7. Separate care consent. This data-processing consent is separate from the informed consent required for a particular health service or treatment. Your Doctor will obtain that care consent when it is needed.

By checking the acceptance box, you confirm that you have read and understood this notice and consent to the processing and controlled sharing described above for the purposes that rely on consent. This consent is specific to this notice version and is recorded with the time and version accepted.$doc$);
