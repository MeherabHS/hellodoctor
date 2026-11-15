-- Peer Navigator documents, accepted at Peer sign-up (all three are required
-- for peers). required=false keeps them out of the Doctor registration
-- required set, exactly as the Patient documents do; Peer registration
-- selects them explicitly. Drafted 2026-11-14 from published peer-navigation
-- practice (CATIE peer health navigation practice guidelines: scope of
-- practice, boundaries and client-confidentiality policies) and the Kenya
-- Data Protection Act 2019 (health data as sensitive personal data, ss. 44-46;
-- children's data, s. 33). Pilot text: the operator must have it reviewed by
-- counsel and add its registered legal identity and DPO contact before public use.
INSERT INTO legal_documents(document_type, version, title, required, is_current, body) VALUES
('PEER_TERMS', '1.0', 'Peer Navigator Terms', false, true, $doc$Peer Navigator Terms (v1.0)

1. Your role. As an AfyaMkononi Peer Navigator you offer non-clinical peer support: listening, sharing your own lived experience where you choose to, encouragement, and help finding and using health services. You are not acting as a doctor, nurse, counsellor or pharmacist. You must not diagnose, prescribe, change anyone's treatment, interpret test results as a clinician would, or give advice that contradicts the person's health-care provider.

2. Approval. You may support people only after an AfyaMkononi Admin approves your account. Approval can be refused, paused (suspended) or withdrawn if these terms or the Peer Navigator Confidentiality, Conduct & Safeguarding Agreement are not followed, or where needed to protect the people using the service.

3. Accurate information. Register with your real name, phone number, email and location, and keep them up to date. Do not share your account or let anyone else use it.

4. Sessions. You create your own support slots. A request appears only after a patient asks for one of your slots; you choose to accept or decline it. Accept only requests you can attend. If you cannot keep an accepted session, tell the person through the app as early as you can.

5. Using the app. Use AfyaMkononi chat and calls for peer support only. Do not move a conversation to personal phone numbers, social media or in-person meetings arranged outside the service. Do not record calls or take screenshots of conversations.

6. Referral to care. When someone needs clinical help, encourage them to book a doctor in the app or attend a health facility. In an emergency, tell them to contact emergency services or go to the nearest facility immediately.

7. No payment or gifts. Peer support through AfyaMkononi is provided without charge to patients. Do not ask for or accept money, gifts or favours from the people you support.

8. Changes. Material changes to these terms are published as a new version, which you will be asked to accept before continuing.

This is the pilot version of these terms. Before public use the operator will publish its registered legal identity and contact details.$doc$),
('PEER_PRIVACY_NOTICE', '1.0', 'Peer Navigator Privacy Notice', false, true, $doc$Peer Navigator Privacy Notice (v1.0)

1. What we hold about you. Your full name, email address, phone number, county, optional organisation and profile text; your account credentials in protected form; your approval status and Admin review notes; the support slots you create; and the requests, messages and call records of the sessions you take part in.

2. Why. To verify and approve Peer Navigators, to let patients choose a peer and book a session, to run chat and calls, to keep the service safe, and to meet legal obligations. Processing relies on your acceptance of the Peer Navigator Terms (contract), our legitimate interest in running a safe service, and legal obligations under Kenyan law.

3. Who sees it. Patients see only your name, county, organisation, profile text and available times. Admins see your account and review details to approve and oversee peers. Your email address and phone number are never shown to patients.

4. Information you see about patients. Patient information, including any health information they choose to share, is sensitive personal data under the Kenya Data Protection Act 2019. You see only a patient's first name and initial, an age range and what they write to you. Use it only to support that person in that session, and keep it confidential as set out in the Confidentiality, Conduct & Safeguarding Agreement.

5. Retention. We keep your information while your account is active and afterwards only as long as required for safety, dispute, audit and legal purposes. Session records are kept on the same basis.

6. Your rights. You may request access to, correction of, or deletion of your personal data, object to or restrict processing, and complain to the Office of the Data Protection Commissioner (ODPC). Some records may be kept where the law requires it.

This is the pilot version of this notice. Before public use the operator will publish its registered identity, Data Protection Officer contact, processor list and hosting locations.$doc$),
('PEER_CONDUCT_SAFEGUARDING', '1.0', 'Peer Navigator Confidentiality, Conduct & Safeguarding Agreement', false, true, $doc$Peer Navigator Confidentiality, Conduct & Safeguarding Agreement (v1.0)

Many people who reach out to a Peer Navigator are adolescents and young people who find it hard to talk to a doctor. This agreement protects them, and you.

1. Confidentiality. Everything a person shares with you, including their HIV status, health, sexuality, relationships and family situation, is confidential. Do not share it with anyone outside AfyaMkononi, including your family, friends, community or other people you support, and do not discuss it on social media. Do not try to find out who someone is beyond what the app shows you. Confidentiality continues after a session ends and after you stop being a Peer Navigator.

2. When confidentiality must be broken. You must tell AfyaMkononi through the app's support channel straight away, sharing only what is needed, if a person: is at immediate risk of serious harm to themselves or others; discloses that a child is being abused, neglected or exploited; or is in a medical emergency. Tell the person, where it is safe to do so, that you need to involve others to keep them safe.

3. Boundaries. Keep the relationship professional and peer-to-peer. Never start a romantic or sexual relationship with, or make sexual comments to, a person you support. Do not lend or borrow money, sell products, promote a religion or political view, or ask for personal contact details. Share your own story only when it helps the other person, and never pressure anyone to disclose.

4. Respect. Treat everyone with dignity regardless of age, gender, sexual orientation, HIV status, disability, religion or background. No judgement, shaming, threats or harassment.

5. Safeguarding young people. Be especially careful with anyone under 18. Do not ask a young person to keep secrets from the service, do not arrange to meet them, and do not contact them outside AfyaMkononi. Keep conversations within the app so they are protected and can be reviewed if a concern is raised.

6. Stay within your role. Do not give clinical advice, medicine or dosing instructions. Encourage the person to see a doctor through the app or attend a health facility, and point them to emergency services in an emergency.

7. Looking after yourself. Supporting others can be hard. If a conversation distresses you, or you feel unsafe or harassed, end the session respectfully and tell AfyaMkononi through the support channel.

8. Consequences. Breaking this agreement may lead to suspension or removal as a Peer Navigator and, where the law requires, a report to the authorities.$doc$);
