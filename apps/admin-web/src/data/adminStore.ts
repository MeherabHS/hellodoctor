export interface DoctorHistoryItem {
  key: string;
  name: string;
  bmdc: string;
  phone: string;
  residence: string;
  email: string;
  avatar: string;
  bgColor: string;
  meta: string;
  kpiVisits: string;
  kpiChats: string;
  kpiGross: string;
  kpiDisbursed: string;
  encounters: {
    time: string;
    ptName: string;
    mode: string;
    badgeCls: string;
    fee: string;
    diagnosis: string;
    rx: string;
  }[];
  disbursements: {
    date: string;
    amount: string;
    method: string;
    txId: string;
    commission: string;
    status: string;
  }[];
}

export const ADMIN_DOCTORS_STORE: Record<string, DoctorHistoryItem> = {
  anika: {
    key: 'anika',
    name: 'Dr. Anika Rahman',
    bmdc: 'BMDC #A-74921',
    phone: '+880 1711-884920',
    residence: 'Dhanmondi, Dhaka (House 42, Road 7A)',
    email: 'dr.anika.dmch@helodoc.com',
    avatar: 'AR',
    bgColor: '#059669',
    meta: 'Pediatric Specialist • Dhaka Medical College Hospital • BMDC Registered Specialist',
    kpiVisits: '1,480',
    kpiChats: '940',
    kpiGross: '৳ 1,020,000',
    kpiDisbursed: '৳ 816,000',
    encounters: [
      { time: 'Today, 11:15 AM', ptName: 'Tanvir Chowdhury (7M)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Pediatric Atopic Dermatitis, Eczema flare', rx: 'Rx #043 Synced' },
      { time: 'Today, 09:30 AM', ptName: 'Zainab Hossain (4F)', mode: '💬 24h Chat Subscribed', badgeCls: 'badge-blue', fee: '৳ 300', diagnosis: 'Acute viral rhinorrhea, saline drops advice', rx: 'Rx #042 Synced' },
      { time: 'Yesterday, 06:10 PM', ptName: 'Sarah Ahmed (34F)', mode: '📹 10m Video + 24h Chat', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Maternal allergy & pediatric immunization plan', rx: 'Rx #040 Synced' },
      { time: '23 Sep, 03:45 PM', ptName: 'Rafiq Ahmed (34M)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Family asthma consultation', rx: 'Rx #036 Synced' }
    ],
    disbursements: [
      { date: '24 Sep 2026', amount: '৳ 62,880', method: 'bKash Merchant', txId: 'BK-8932402', commission: '৳ 15,720 (20%)', status: 'Settled ✓' },
      { date: '17 Sep 2026', amount: '৳ 48,000', method: 'bKash Merchant', txId: 'BK-8821941', commission: '৳ 12,000 (20%)', status: 'Settled ✓' }
    ]
  },
  sadik: {
    key: 'sadik',
    name: 'Dr. Sadik Al-Amin',
    bmdc: 'BMDC #A-68192',
    phone: '+880 1819-334455',
    residence: 'Gulshan-2, Dhaka (Avenue 3, Block C)',
    email: 'dr.sadik.bsmmu@helodoc.com',
    avatar: 'SA',
    bgColor: '#2563EB',
    meta: 'General Medicine & Diabetology • BSMMU • BMDC Registered Specialist',
    kpiVisits: '2,150',
    kpiChats: '1,420',
    kpiGross: '৳ 1,480,000',
    kpiDisbursed: '৳ 1,184,000',
    encounters: [
      { time: 'Today, 10:45 AM', ptName: 'Rafiq Ahmed (34M)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Acute URTI, Severe Pharyngitis follow-up', rx: 'Rx #041 Synced' },
      { time: 'Today, 09:15 AM', ptName: 'Kamal Uddin (52M)', mode: '💬 24h Chat Subscribed', badgeCls: 'badge-blue', fee: '৳ 300', diagnosis: 'Hypertension titration & Metformin review', rx: 'Rx #038 Synced' },
      { time: 'Yesterday, 04:30 PM', ptName: 'Farzana Haque (41F)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Bronchial asthma inhaler adjustment', rx: 'Rx #037 Synced' },
      { time: '22 Sep, 11:20 AM', ptName: 'Nusrat Jahan (28F)', mode: '💬 24h Chat Subscribed', badgeCls: 'badge-blue', fee: '৳ 300', diagnosis: 'Thyroid profile interpretation', rx: 'Rx #034 Synced' }
    ],
    disbursements: [
      { date: '24 Sep 2026', amount: '৳ 84,000', method: 'Nagad Business', txId: 'NG-894102', commission: '৳ 21,000 (20%)', status: 'Settled ✓' }
    ]
  },
  farhana: {
    key: 'farhana',
    name: 'Dr. Farhana Yesmin',
    bmdc: 'BMDC #A-53419',
    phone: '+880 1912-778899',
    residence: 'Uttara Sector 4, Dhaka (Road 11)',
    email: 'dr.farhana.birdem@helodoc.com',
    avatar: 'FY',
    bgColor: '#7C3AED',
    meta: 'Gynaecology & Obstetrics • BIRDEM Hospital • BMDC Registered Specialist',
    kpiVisits: '1,890',
    kpiChats: '1,110',
    kpiGross: '৳ 1,320,000',
    kpiDisbursed: '৳ 1,056,000',
    encounters: [
      { time: 'Today, 11:30 AM', ptName: 'Nusrat Jahan (28F)', mode: '📹 10m Video + 24h Chat', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Hypothyroidism & Antenatal routine review', rx: 'Rx #044 Synced' },
      { time: 'Today, 10:00 AM', ptName: 'Sarah Ahmed (34F)', mode: '💬 24h Chat Subscribed', badgeCls: 'badge-blue', fee: '৳ 300', diagnosis: 'Post-partum iron deficiency screening', rx: 'Rx #039 Synced' },
      { time: 'Yesterday, 05:20 PM', ptName: 'Farzana Haque (41F)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Hormonal panel evaluation', rx: 'Rx #035 Synced' }
    ],
    disbursements: [
      { date: '24 Sep 2026', amount: '৳ 55,200', method: 'BEFTN Bank', txId: 'BNK-77192', commission: '৳ 13,800 (20%)', status: 'Settled ✓' }
    ]
  },
  sabrina: {
    key: 'sabrina',
    name: 'Dr. Sabrina Akter',
    bmdc: 'BMDC #45821',
    phone: '+880 1713-445566',
    residence: 'Banani, Dhaka (Road 11, Block D)',
    email: 'dr.sabrina.apollo@helodoc.com',
    avatar: 'SA',
    bgColor: '#D97706',
    meta: 'Internal Medicine Consultant • Apollo Hospitals Dhaka • BMDC Registered Specialist',
    kpiVisits: '1,240',
    kpiChats: '860',
    kpiGross: '৳ 890,000',
    kpiDisbursed: '৳ 712,000',
    encounters: [
      { time: 'Today, 10:30 AM', ptName: 'Rafiq Ahmed (34M)', mode: '📹 10m Video + 24h Chat', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Acute URTI, Severe Pharyngitis', rx: 'Rx #041 Synced' },
      { time: 'Today, 10:40 AM', ptName: 'Nusrat Jahan (28F)', mode: '📹 10m Video + 24h Chat', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Hypothyroidism, Routine Review', rx: 'Rx #039 Synced' },
      { time: 'Today, 10:50 AM', ptName: 'Kamal Uddin (52M)', mode: '💬 24h Chat Subscribed', badgeCls: 'badge-blue', fee: '৳ 300', diagnosis: 'Hypertension Maintenance & Refill', rx: 'Rx #038 Synced' },
      { time: 'Yesterday, 04:15 PM', ptName: 'Tariqul Islam (46M)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Dyspepsia, Acid Reflux', rx: 'Rx #035 Synced' }
    ],
    disbursements: [
      { date: '24 Sep 2026', amount: '৳ 45,200', method: 'bKash Merchant', txId: 'BK-8932401', commission: '৳ 11,300 (20%)', status: 'Settled ✓' },
      { date: '17 Sep 2026', amount: '৳ 38,500', method: 'bKash Merchant', txId: 'BK-8821940', commission: '৳ 9,625 (20%)', status: 'Settled ✓' }
    ]
  },
  karim: {
    key: 'karim',
    name: 'Dr. Karim Hossain',
    bmdc: 'BMDC #81551',
    phone: '+880 1718-223344',
    residence: 'Dhanmondi, Dhaka (Road 4)',
    email: 'dr.karim.cardio@helodoc.com',
    avatar: 'KH',
    bgColor: '#1D4ED8',
    meta: 'Cardiology Specialist • National Heart Institute • BMDC Registered Specialist',
    kpiVisits: '980',
    kpiChats: '620',
    kpiGross: '৳ 680,000',
    kpiDisbursed: '৳ 544,000',
    encounters: [
      { time: 'Today, 10:40 AM', ptName: 'Tanvir Hasan (45M)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Post-MI ECG Evaluation', rx: 'Rx #037 Synced' },
      { time: 'Yesterday, 03:00 PM', ptName: 'Farzana Haque (41F)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'Palpitations & Holter check', rx: 'Rx #035 Synced' }
    ],
    disbursements: [
      { date: '24 Sep 2026', amount: '৳ 32,000', method: 'bKash Merchant', txId: 'BK-891044', commission: '৳ 8,000 (20%)', status: 'Settled ✓' }
    ]
  },
  tariqul: {
    key: 'tariqul',
    name: 'Dr. Tariqul Islam',
    bmdc: 'BMDC #62410',
    phone: '+880 1812-445566',
    residence: 'Banani, Dhaka (Road 2)',
    email: 'dr.tariqul.digest@helodoc.com',
    avatar: 'TI',
    bgColor: '#047857',
    meta: 'Gastroenterology Consultant • Square Hospital • BMDC Registered Specialist',
    kpiVisits: '1,120',
    kpiChats: '740',
    kpiGross: '৳ 820,000',
    kpiDisbursed: '৳ 656,000',
    encounters: [
      { time: 'Yesterday, 04:15 PM', ptName: 'Rafiq Ahmed (34M)', mode: '📹 10m Video Visit', badgeCls: 'badge-green', fee: '৳ 800', diagnosis: 'GERD & H. pylori assessment', rx: 'Rx #033 Synced' }
    ],
    disbursements: [
      { date: '24 Sep 2026', amount: '৳ 42,400', method: 'Nagad Business', txId: 'NG-881920', commission: '৳ 10,600 (20%)', status: 'Settled ✓' }
    ]
  }
};

export interface PatientRecordItem {
  key: string;
  name: string;
  id: string;
  chatActive: boolean;
  avatar: string;
  bgColor: string;
  meta: string;
  bp: string;
  pulse: string;
  cohort: string;
  labs: string;
  category: string;
  encounters: {
    date: string;
    doctor: string;
    spec: string;
    mode: string;
    badgeCls: string;
    diag: string;
    rx: string;
  }[];
}

export const ADMIN_PATIENT_STORE: Record<string, PatientRecordItem> = {
  sarah: {
    key: 'sarah',
    name: 'Sarah Ahmed',
    id: '#PT-6521',
    chatActive: true,
    avatar: 'SA',
    bgColor: '#BE185D',
    meta: '34 F • Dhanmondi, Dhaka • Registered: Jan 2025 • Lifetime Spend: ৳ 75,000',
    bp: '138/88 mmHg',
    pulse: '76 bpm',
    cohort: 'Hypertension • T2DM',
    labs: '3 Documents in Vault',
    category: 'chronic',
    encounters: [
      { date: '15 Oct 2026', doctor: 'Dr. Kamal Uddin', spec: 'General Practice', mode: '📹 Video Visit', badgeCls: 'badge-green', diag: 'Hypertension follow-up, Metformin review', rx: 'Rx #041' },
      { date: '28 Sep 2026', doctor: 'Dr. Sabrina Akter', spec: 'Internal Medicine', mode: '💬 24h Chat Only', badgeCls: 'badge-blue', diag: 'Olmesartan dosage titration', rx: 'Rx #038' },
      { date: '10 Sep 2026', doctor: 'Dr. Karim Hossain', spec: 'Cardiology', mode: '📹 Video Visit', badgeCls: 'badge-green', diag: 'ECG baseline check & lifestyle counseling', rx: 'Rx #032' }
    ]
  },
  rafiq: {
    key: 'rafiq',
    name: 'Rafiq Ahmed',
    id: '#PT-6522',
    chatActive: true,
    avatar: 'RA',
    bgColor: '#1D4ED8',
    meta: '34 M • Dhanmondi Rd 4 • Registered: Mar 2024 • Lifetime Spend: ৳ 42,000',
    bp: '122/78 mmHg',
    pulse: '72 bpm',
    cohort: 'Acute URTI • Hypertension',
    labs: '4 Documents in Vault',
    category: 'chronic',
    encounters: [
      { date: 'Today, 10:30 AM', doctor: 'Dr. Sabrina Akter', spec: 'Internal Medicine', mode: '📹 Video + 24h Chat', badgeCls: 'badge-green', diag: 'Acute URTI, Severe Pharyngitis', rx: 'Rx #041' },
      { date: '14 Oct 2026', doctor: 'Dr. Sadik Al-Amin', spec: 'General Medicine', mode: '💬 24h Chat Only', badgeCls: 'badge-blue', diag: 'Pre-hypertension lifestyle diet check', rx: 'Rx #037' },
      { date: '02 Oct 2026', doctor: 'Dr. Anika Rahman', spec: 'Preventive Care', mode: '📹 Video Visit', badgeCls: 'badge-green', diag: 'Annual wellness checkup', rx: 'Rx #033' }
    ]
  },
  nusrat: {
    key: 'nusrat',
    name: 'Nusrat Jahan',
    id: '#PT-6523',
    chatActive: true,
    avatar: 'NJ',
    bgColor: '#7E22CE',
    meta: '28 F • Banani Block C • Registered: Jun 2025 • Lifetime Spend: ৳ 42,000',
    bp: '115/74 mmHg',
    pulse: '78 bpm',
    cohort: 'Hypothyroidism • PCOD',
    labs: '2 Documents in Vault',
    category: 'chronic',
    encounters: [
      { date: 'Today, 10:40 AM', doctor: 'Dr. Sabrina Akter', spec: 'Internal Medicine', mode: '📹 Video + 24h Chat', badgeCls: 'badge-green', diag: 'Hypothyroidism, Routine Review', rx: 'Rx #039' },
      { date: '22 Sep 2026', doctor: 'Dr. Farhana Yesmin', spec: 'Gynaecology', mode: '💬 24h Chat Only', badgeCls: 'badge-blue', diag: 'Thyroid profile & cycle evaluation', rx: 'Rx #034' }
    ]
  },
  kamal: {
    key: 'kamal',
    name: 'Kamal Uddin',
    id: '#PT-6524',
    chatActive: true,
    avatar: 'KU',
    bgColor: '#B45309',
    meta: '52 M • Mirpur / Uttara • Registered: Nov 2023 • Lifetime Spend: ৳ 92,000',
    bp: '142/90 mmHg',
    pulse: '82 bpm',
    cohort: 'Hypertension • T2DM Cohort',
    labs: '6 Documents in Vault',
    category: 'chat chronic',
    encounters: [
      { date: 'Today, 10:50 AM', doctor: 'Dr. Sabrina Akter', spec: 'Internal Medicine', mode: '💬 24h Chat Subscribed', badgeCls: 'badge-blue', diag: 'Hypertension Maintenance & Refill', rx: 'Rx #038' },
      { date: '10 Oct 2026', doctor: 'Dr. Sadik Al-Amin', spec: 'Diabetology', mode: '📹 Video Visit', badgeCls: 'badge-green', diag: 'HbA1c quarterly monitoring (7.2%)', rx: 'Rx #036' }
    ]
  },
  farzana: {
    key: 'farzana',
    name: 'Farzana Haque',
    id: '#PT-6525',
    chatActive: false,
    avatar: 'FH',
    bgColor: '#0369A1',
    meta: '41 F • Gulshan 2 • Registered: Feb 2025 • Lifetime Spend: ৳ 38,000',
    bp: '126/80 mmHg',
    pulse: '74 bpm',
    cohort: 'Bronchial Asthma',
    labs: '3 Documents in Vault',
    category: 'chronic',
    encounters: [
      { date: 'Yesterday', doctor: 'Dr. Karim Hossain', spec: 'Pulmonology', mode: '📹 Video Visit', badgeCls: 'badge-green', diag: 'Bronchial Asthma exacerbation management', rx: 'Rx #035' },
      { date: '08 Oct 2026', doctor: 'Dr. Sadik Al-Amin', spec: 'General Medicine', mode: '📹 Video Visit', badgeCls: 'badge-green', diag: 'Seasonal allergy & antihistamines', rx: 'Rx #031' }
    ]
  },
  tanvir: {
    key: 'tanvir',
    name: 'Tanvir Chowdhury',
    id: '#PT-6526',
    chatActive: true,
    avatar: 'TC',
    bgColor: '#059669',
    meta: '7 M (Child) • Baridhara, Dhaka • Registered: Aug 2025 • Lifetime Spend: ৳ 21,500',
    bp: '105/68 mmHg',
    pulse: '92 bpm',
    cohort: 'Pediatric Atopic Dermatitis',
    labs: '1 Document in Vault',
    category: 'pediatric',
    encounters: [
      { date: 'Today, 11:15 AM', doctor: 'Dr. Anika Rahman', spec: 'Pediatrics', mode: '📹 Video Visit', badgeCls: 'badge-green', diag: 'Pediatric Atopic Dermatitis, Topical emollient refill', rx: 'Rx #043' },
      { date: '12 Oct 2026', doctor: 'Dr. Anika Rahman', spec: 'Pediatrics', mode: '💬 24h Chat Only', badgeCls: 'badge-blue', diag: 'Fever review post-vaccination', rx: 'Rx #030' }
    ]
  }
};

export interface AppErrorLogItem {
  id: string;
  traceId: string;
  timestamp: string;
  timeFormatted: string;
  relativeTime: string;
  user: {
    id: string;
    name: string;
    ageGender: string;
    role: string;
    phone: string;
    avatar: string;
    avatarBg: string;
    device: string;
    appVersion: string;
    network: string;
    ip: string;
  };
  severity: 'CRITICAL' | 'ERROR' | 'WARNING' | 'DEGRADED';
  subsystem: string;
  component: string;
  actionAttempted: string;
  failedPart: string;
  errorCode: string;
  errorMessage: string;
  stackTrace: string;
  status: 'UNRESOLVED' | 'INVESTIGATING' | 'RESOLVED';
  resolvedAt: string | null;
  resolutionNote: string | null;
}

export const INITIAL_ERROR_LOGS: AppErrorLogItem[] = [
  {
    id: 'ERR-20260930-01',
    traceId: 'TRC-94812-BKASH',
    timestamp: '2026-09-30T15:32:14.280Z',
    timeFormatted: 'Today, 15:32:14',
    relativeTime: '4 mins ago',
    user: {
      id: 'USR-8921',
      name: 'Sarah Khan',
      ageGender: '29F',
      role: 'Patient',
      phone: '+880 1711-234567',
      avatar: 'SK',
      avatarBg: '#059669',
      device: 'Samsung Galaxy S24 • Android 14',
      appVersion: 'HeloDoc Patient v2.4.1 (Build 184)',
      network: 'Grameenphone 4G (RTT 380ms, 12% Loss)',
      ip: '103.114.98.24 (Dhaka Central)'
    },
    severity: 'CRITICAL',
    subsystem: 'Payment Gateway',
    component: '/api/v2/payment/bkash/execute-agreement',
    actionAttempted: 'Patient Consultation Checkout (৳800 for Dr. Sabrina)',
    failedPart: 'bKash Merchant Escrow Callback Handshake',
    errorCode: 'HTTP 504 GATEWAY_TIMEOUT',
    errorMessage: 'Upstream bKash PGW did not respond within 15,000ms timeout threshold during tokenized debit execution.',
    stackTrace: 'Error: GatewayTimeoutException [504]\n    at BkashPaymentAdapter.executePayment (src/adapters/payment/bkash.rs:184:12)\n    at PaymentOrchestrator.debitConsultationEscrow (src/services/billing.rs:92:8)\n    at async ActixWebHandler.handleCheckout (src/controllers/checkout.rs:45:19)\n[Context]: booking_ref="BK-SA-8942", amount=800, currency="BDT", gateway_trace="bkash_gw_node_dhk_04"',
    status: 'UNRESOLVED',
    resolvedAt: null,
    resolutionNote: null
  },
  {
    id: 'ERR-20260930-02',
    traceId: 'TRC-94811-WEBRTC',
    timestamp: '2026-09-30T15:18:02.114Z',
    timeFormatted: 'Today, 15:18:02',
    relativeTime: '18 mins ago',
    user: {
      id: 'DOC-45821',
      name: 'Dr. Sabrina Akter',
      ageGender: 'Specialist',
      role: 'Doctor',
      phone: '+880 1819-334455',
      avatar: 'SA',
      avatarBg: '#2563EB',
      device: 'MacBook Pro M3 • Chrome 128 / macOS 14.5',
      appVersion: 'HeloDoc Doctor Web v2.4.0',
      network: 'Hospital Fiber LAN (Proxy / Firewall)',
      ip: '103.205.71.18 (Apollo Dhaka)'
    },
    severity: 'CRITICAL',
    subsystem: 'Agora WebRTC',
    component: "agora.joinChannel('room_hd_7894', uid: 45821)",
    actionAttempted: 'Join 10-Minute Video Consultation with Rafiq Ahmed',
    failedPart: 'Agora WebRTC ICE Candidate Pairing & Media Relay',
    errorCode: 'ICE_FAILED (Code 702)',
    errorMessage: 'Interactive Connectivity Establishment failed on TURN relay (turn.dhaka-cluster.helodoc.net:3478). UDP transport blocked by hospital firewall.',
    stackTrace: 'AgoraRTCException: [702] ICE Connection State: Disconnected\n    at RtcEngine.handleIceFailure (agora-rtc-sdk.js:1420:8)\n    at PeerConnection.onIceConnectionChange (agora-rtc-sdk.js:890:14)\n[Relay Candidates]: 103.205.71.18:54210 -> turn.dhaka-cluster:3478 [TIMEOUT after 5 retries]',
    status: 'UNRESOLVED',
    resolvedAt: null,
    resolutionNote: null
  },
  {
    id: 'ERR-20260930-03',
    traceId: 'TRC-94788-SYNC',
    timestamp: '2026-09-30T14:45:22.090Z',
    timeFormatted: 'Today, 14:45:22',
    relativeTime: '51 mins ago',
    user: {
      id: 'USR-7410',
      name: 'Rafiq Ahmed',
      ageGender: '34M',
      role: 'Patient',
      phone: '+880 1819-987654',
      avatar: 'RA',
      avatarBg: '#059669',
      device: 'Xiaomi Redmi Note 13 • Android 13',
      appVersion: 'HeloDoc Patient v2.4.1 (Build 184)',
      network: 'Robi 3G Edge (Intermittent Signal)',
      ip: '119.30.38.102 (Chittagong)'
    },
    severity: 'ERROR',
    subsystem: 'Sync & Offline Cache',
    component: 'SyncEngine.flushPendingMutations()',
    actionAttempted: 'Synchronize offline chat thread reply to Dr. Sabrina',
    failedPart: 'Offline Mutation Sync Queue Flush',
    errorCode: 'SYNC_TOKEN_EXPIRED (401)',
    errorMessage: 'Client cached 3 chat messages while offline; JWT access token expired during background sync attempt upon reconnecting.',
    stackTrace: 'SyncException: [401] AuthTokenExpiredDuringOfflineDwell\n    at SQLiteQueue.flushPendingMutations (src/offline/sync_worker.ts:114:9)\n    at AuthInterceptor.refreshTokenAndRetry (src/network/client.ts:88:5)',
    status: 'UNRESOLVED',
    resolvedAt: null,
    resolutionNote: null
  },
  {
    id: 'ERR-20260930-04',
    traceId: 'TRC-94765-DGDA',
    timestamp: '2026-09-30T13:12:09.521Z',
    timeFormatted: 'Today, 13:12:09',
    relativeTime: '2 hrs ago',
    user: {
      id: 'DOC-74921',
      name: 'Dr. Anika Rahman',
      ageGender: 'Specialist',
      role: 'Doctor',
      phone: '+880 1712-445566',
      avatar: 'AR',
      avatarBg: '#D97706',
      device: 'iPhone 15 Pro • iOS 17.5',
      appVersion: 'HeloDoc Doctor Mobile v2.3.9',
      network: 'Banglalink 4G (RTT 62ms)',
      ip: '103.88.232.14 (Dhaka)'
    },
    severity: 'ERROR',
    subsystem: 'DGDA Clinical Engine',
    component: '/api/v1/prescriptions/audit-and-sign',
    actionAttempted: 'Sign and dispatch digital prescription Rx #HD-891042',
    failedPart: 'DGDA Narcotic Drug Rule Validation Engine',
    errorCode: 'DGDA_VALIDATION_ERR (422)',
    errorMessage: 'Schedule H Controlled Substance (Inj. Morphine 10mg) cannot be prescribed via telemedicine without emergency clinical ops counter-signature.',
    stackTrace: 'ClinicalValidationError: [422] DGDA Schedule H Rule\n    at DgdaSafetyFilter.validateNarcotics (src/clinical/dgda_rules.rs:412:15)\n    at PrescriptionService.signPrescription (src/clinical/rx_service.rs:184:10)',
    status: 'INVESTIGATING',
    resolvedAt: null,
    resolutionNote: 'Flagged for review in Rx Audit tab. Awaiting clinical director review.'
  },
  {
    id: 'ERR-20260930-05',
    traceId: 'TRC-94710-TELCO',
    timestamp: '2026-09-30T11:24:40.812Z',
    timeFormatted: 'Today, 11:24:40',
    relativeTime: '4 hrs ago',
    user: {
      id: 'USR-6218',
      name: 'Kamal Uddin',
      ageGender: '52M',
      role: 'Patient',
      phone: '+880 1912-345678',
      avatar: 'KU',
      avatarBg: '#059669',
      device: 'Samsung Galaxy A54 • Android 14',
      appVersion: 'HeloDoc Patient v2.4.1',
      network: 'WiFi Broadband',
      ip: '103.145.118.9'
    },
    severity: 'WARNING',
    subsystem: 'Auth & OTP Gateway',
    component: '/api/v1/auth/otp/send-sms',
    actionAttempted: 'Two-Factor Login Authentication',
    failedPart: 'SMS OTP Delivery Carrier Gateway (Banglalink)',
    errorCode: 'TELCO_ROUTE_CONGESTED (429)',
    errorMessage: 'Banglalink DLR route reporting >120s queue latency. Fallback routing to GP Telco aggregator initiated automatically.',
    stackTrace: 'TelcoCarrierException: [429] Route Congested\n    at TelcoRouter.dispatchOtp (src/auth/sms_gateway.rs:74:14)\n    at AuthService.generateAndSendOtp (src/auth/service.rs:102:9)',
    status: 'RESOLVED',
    resolvedAt: 'Today, 11:25:02',
    resolutionNote: 'Auto-resolved via circuit breaker fallback to secondary Grameenphone aggregator.'
  },
  {
    id: 'ERR-20260930-06',
    traceId: 'TRC-94645-OCR',
    timestamp: '2026-09-29T19:40:15.340Z',
    timeFormatted: 'Yesterday, 19:40:15',
    relativeTime: '19 hrs ago',
    user: {
      id: 'USR-5190',
      name: 'Nusrat Jahan',
      ageGender: '28F',
      role: 'Patient',
      phone: '+880 1611-998877',
      avatar: 'NJ',
      avatarBg: '#059669',
      device: 'Vivo V29 • Android 13',
      appVersion: 'HeloDoc Patient v2.4.0',
      network: 'WiFi 5GHz',
      ip: '103.114.98.55'
    },
    severity: 'WARNING',
    subsystem: 'Sync & Offline Cache',
    component: 'Worker.processLabReportPdf(docId: LAB-THY-092)',
    actionAttempted: 'Upload Thyroid Panel Diagnostic Report PDF',
    failedPart: 'Diagnostic Lab PDF Report OCR Pipeline',
    errorCode: 'IMAGE_DPI_SUBPAR (206)',
    errorMessage: 'Patient uploaded 72 DPI smartphone photo; OCR confidence 64% below 85% clinical safety threshold. Flagged for manual technician verification.',
    stackTrace: 'OcrQualityException: [206] LowDpiArtifacts\n    at DocumentParser.extractBiomarkers (src/emr/ocr_engine.py:128:8)\n    at LabVaultService.ingestReport (src/emr/service.py:94:12)',
    status: 'RESOLVED',
    resolvedAt: 'Yesterday, 20:05:00',
    resolutionNote: 'Lab technician manually transcribed TSH & FT4 values into patient EMR vault.'
  },
  {
    id: 'ERR-20260930-07',
    traceId: 'TRC-94590-BMDC',
    timestamp: '2026-09-29T16:05:31.902Z',
    timeFormatted: 'Yesterday, 16:05:31',
    relativeTime: '23 hrs ago',
    user: {
      id: 'DOC-51820',
      name: 'Dr. Sadik Hasan',
      ageGender: 'Specialist',
      role: 'Doctor',
      phone: '+880 1715-889900',
      avatar: 'SH',
      avatarBg: '#2563EB',
      device: 'Windows 11 • Edge 127',
      appVersion: 'HeloDoc Doctor Web v2.4.0',
      network: 'Broadband Fiber (Dhaka)',
      ip: '103.48.26.110'
    },
    severity: 'DEGRADED',
    subsystem: 'BMDC KYC Hub',
    component: 'BmdcRegistryClient.lookupDoctor(A-45821)',
    actionAttempted: 'Real-time BMDC credential validation during profile update',
    failedPart: 'BMDC Regulatory Registry Scraping & Cache Refresh',
    errorCode: 'BMDC_PORTAL_MAINTENANCE (503)',
    errorMessage: 'Official government portal returned HTTP 503 Service Unavailable. Fallback to cached verified snapshot v2026.09.28.',
    stackTrace: 'HttpServiceException: [503] Gateway Down\n    at BmdcRegistryClient.fetchPhysician (src/kyc/bmdc_scraper.py:84:10)\n    at KycWorkflow.validateSpecialist (src/kyc/orchestrator.py:32:8)',
    status: 'RESOLVED',
    resolvedAt: 'Yesterday, 16:30:15',
    resolutionNote: 'Government server restored; cache revalidated with 0 discrepancies.'
  }
];

export interface GrievanceItem {
  id: string;
  timestamp: string;
  timeFormatted: string;
  relativeTime: string;
  patient: {
    id: string;
    name: string;
    phone: string;
    ageGender: string;
  };
  doctor: {
    id: string;
    name: string;
    bmdc: string;
    specialty: string;
    hospital: string;
    fee: string;
  };
  consultationId: string;
  consultationType: string;
  target: 'DOCTOR' | 'SYSTEM';
  category: string;
  severity: 'CRITICAL' | 'HIGH' | 'MEDIUM';
  claimSummary: string;
  patientStatement: string;
  boardRemedy: string;
  telemetryEvidence: {
    callDuration: string;
    connectionStatus: string;
    rxIssued: string;
    paymentStatus: string;
    webrtcDiagnostics?: {
      iceConnectionState: string;
      audioQuality: string;
      videoQuality: string;
      packetLossPercent: string;
      networkPingRtt: string;
      jitter: string;
      reconnectCount: number;
    };
    deviceInfo?: string;
    escrowStatus?: string;
    auditTimeline?: { time: string; event: string }[];
  };
  status: 'PENDING_REVIEW' | 'INVESTIGATING' | 'REFUNDED' | 'WARNED' | 'RESOLVED';
  adjudication: {
    action: string;
    date: string;
    admin: string;
    notes: string;
  } | null;
}

export const INITIAL_GRIEVANCES: GrievanceItem[] = [
  {
    id: 'GRV-20260930-01',
    timestamp: '2026-09-30T15:24:10.000Z',
    timeFormatted: 'Today, 15:24:10',
    relativeTime: '20 mins ago',
    patient: {
      id: 'USR-8921',
      name: 'Sarah Khan',
      phone: '+880 1711-234567',
      ageGender: '29F'
    },
    doctor: {
      id: 'DOC-45821',
      name: 'Dr. Sabrina Akter',
      bmdc: 'BMDC #45821',
      specialty: 'Internal Medicine Consultant',
      hospital: 'Apollo Hospitals Dhaka',
      fee: '৳800'
    },
    consultationId: 'CONS-9481',
    consultationType: 'Video Consultation (10 min)',
    target: 'DOCTOR',
    category: 'Rushed Consultation / Ended Abruptly',
    severity: 'HIGH',
    claimSummary: 'Doctor terminated video call after only 2 minutes without listening to symptoms or prescribing medication.',
    patientStatement: 'I paid ৳800 for a 10-minute specialist consultation. Dr. Sabrina joined late, stayed for 2 minutes and 15 seconds, and told me to just visit her private clinic physically. She refused to write an e-Prescription. This is an injustice and waste of money.',
    boardRemedy: 'Under Governance Review',
    telemetryEvidence: {
      callDuration: '2m 14s (Expected: 10m)',
      connectionStatus: 'Stable 4G (0% packet loss)',
      rxIssued: 'Not Issued',
      paymentStatus: 'Settled ৳800 via bKash',
      webrtcDiagnostics: {
        iceConnectionState: 'Connected (STUN/TURN Pair Succeeded)',
        audioQuality: '32 kbps (Opus)',
        videoQuality: '1080p (1.8 Mbps H.264)',
        packetLossPercent: '0.0%',
        networkPingRtt: '38ms',
        jitter: '12ms',
        reconnectCount: 0
      },
      deviceInfo: 'Samsung Galaxy A54 • Android 14 • Grameenphone 4G',
      escrowStatus: 'Held in HeloDoc Escrow Gateway',
      auditTimeline: [
        { time: '10:30:00', event: 'Escrow Lock Confirmed (৳800 debited from bKash)' },
        { time: '10:30:04', event: 'WebRTC Room Initialized (ICE connected)' },
        { time: '10:30:12', event: 'Dr. Sabrina Akter joined video session' },
        { time: '10:32:14', event: 'Doctor terminated call prematurely at 2m 14s' },
        { time: '10:32:15', event: 'Prescription Vault Check: Refused / Not Created' }
      ]
    },
    status: 'PENDING_REVIEW',
    adjudication: null
  },
  {
    id: 'GRV-20260930-02',
    timestamp: '2026-09-30T14:48:32.000Z',
    timeFormatted: 'Today, 14:48:32',
    relativeTime: '55 mins ago',
    patient: {
      id: 'USR-6192',
      name: 'Rafiq Ahmed',
      phone: '+880 1712-345678',
      ageGender: '32M'
    },
    doctor: {
      id: 'DOC-39410',
      name: 'Dr. Tanvir Ahmed',
      bmdc: 'BMDC #39410',
      specialty: 'Pediatrics & Child Health',
      hospital: 'Dhaka Shishu Hospital',
      fee: '৳650'
    },
    consultationId: 'CONS-9477',
    consultationType: 'Video Consultation (10 min)',
    target: 'SYSTEM',
    category: 'WebRTC Video / Audio Freeze',
    severity: 'CRITICAL',
    claimSummary: 'Screen froze entirely during video call with Dr. Tanvir. System debited ৳650 but call could not connect.',
    patientStatement: 'My 4-year-old child had high fever. The video connected for 5 seconds and froze with a black screen and ICE candidate failure. We tried reconnecting 3 times, but it kept failing. ৳650 was debited from my Nagad account, but we received no medical advice.',
    boardRemedy: 'Technical Telemetry Inquest',
    telemetryEvidence: {
      callDuration: '0m 08s (Expected: 10m)',
      connectionStatus: 'ICE Failed (WebRTC Code 702)',
      rxIssued: 'Not Issued',
      paymentStatus: 'Settled ৳650 via Nagad',
      webrtcDiagnostics: {
        iceConnectionState: 'Failed (WebRTC Code 702 - NAT Traversal Timeout)',
        audioQuality: 'Dropped (0 kbps)',
        videoQuality: 'Stream Frozen (0 kbps)',
        packetLossPercent: '89.4%',
        networkPingRtt: 'Timeout (> 2500ms)',
        jitter: '420ms',
        reconnectCount: 3
      },
      deviceInfo: 'Xiaomi Redmi Note 12 • Android 13 • Banglalink 4G',
      escrowStatus: 'Held in HeloDoc Escrow Gateway',
      auditTimeline: [
        { time: '14:48:00', event: 'Payment confirmed via Nagad (৳650)' },
        { time: '14:48:05', event: 'WebRTC Room Connection Started' },
        { time: '14:48:13', event: 'ICE Candidate Failure (Code 702) - Stream Frozen' },
        { time: '14:48:18', event: 'Auto-reconnect attempt 1 failed' },
        { time: '14:48:25', event: 'Session severed due to network gateway timeout' }
      ]
    },
    status: 'INVESTIGATING',
    adjudication: null
  },
  {
    id: 'GRV-20260930-03',
    timestamp: '2026-09-30T11:15:00.000Z',
    timeFormatted: 'Today, 11:15:00',
    relativeTime: '4 hrs ago',
    patient: {
      id: 'USR-4819',
      name: 'Kamal Uddin',
      phone: '+880 1819-776655',
      ageGender: '52M'
    },
    doctor: {
      id: 'DOC-68192',
      name: 'Dr. Sadik Al-Amin',
      bmdc: 'BMDC #A-68192',
      specialty: 'General Medicine & Diabetology',
      hospital: 'BSMMU',
      fee: '৳800'
    },
    consultationId: 'CONS-9462',
    consultationType: '24h Chat Consultation',
    target: 'DOCTOR',
    category: 'Refused e-Prescription',
    severity: 'HIGH',
    claimSummary: 'Physician refused to issue digital prescription after taking ৳300 chat fee, demanding separate video appointment.',
    patientStatement: 'I sent my fasting glucose blood reports and explained my hypertension history. Dr. Sadik replied with two short words and told me he will not write a prescription unless I pay again for video consultation.',
    boardRemedy: 'BMDC Conduct Inquiry',
    telemetryEvidence: {
      callDuration: 'Chat Session (2 messages by Doc)',
      connectionStatus: 'Online (Broadband)',
      rxIssued: 'Refused by Physician',
      paymentStatus: 'Settled ৳300 via bKash',
      webrtcDiagnostics: {
        iceConnectionState: 'N/A (Encrypted Chat Socket Active)',
        audioQuality: 'N/A',
        videoQuality: 'N/A',
        packetLossPercent: '0.0%',
        networkPingRtt: '24ms',
        jitter: '2ms',
        reconnectCount: 0
      },
      deviceInfo: 'Samsung Galaxy M34 • Android 14 • WiFi',
      escrowStatus: 'Held in HeloDoc Escrow Gateway',
      auditTimeline: [
        { time: '11:15:00', event: '24h Chat Session activated, ৳300 locked in escrow' },
        { time: '11:18:22', event: 'Patient uploaded lab report (Fasting Glucose)' },
        { time: '11:22:40', event: 'Doctor sent message: "Visit chamber or pay for video"' },
        { time: '11:24:00', event: 'Prescription Vault: Refused to issue' }
      ]
    },
    status: 'PENDING_REVIEW',
    adjudication: null
  },
  {
    id: 'GRV-20260929-04',
    timestamp: '2026-09-29T18:10:44.000Z',
    timeFormatted: 'Yesterday, 18:10:44',
    relativeTime: '21 hrs ago',
    patient: {
      id: 'USR-3401',
      name: 'Nusrat Jahan',
      phone: '+880 1912-998877',
      ageGender: '28F'
    },
    doctor: {
      id: 'DOC-53419',
      name: 'Dr. Farhana Yesmin',
      bmdc: 'BMDC #A-53419',
      specialty: 'Gynaecology & Obstetrics',
      hospital: 'BIRDEM Hospital',
      fee: '৳800'
    },
    consultationId: 'CONS-9430',
    consultationType: 'Video Consultation (10 min)',
    target: 'SYSTEM',
    category: 'bKash Payment Debited But Session Failed',
    severity: 'CRITICAL',
    claimSummary: 'Double debit occurred on bKash during payment timeout; both transactions debited ৳800 from patient wallet.',
    patientStatement: 'During payment, the bKash screen timed out with an error. I paid again, but checking my bKash statement revealed both payments of ৳800 were deducted (Total ৳1,600).',
    boardRemedy: 'Escrow Refund Disbursed (৳800)',
    telemetryEvidence: {
      callDuration: '10m 00s (Consultation completed)',
      connectionStatus: 'Completed Normal',
      rxIssued: 'Rx #044 Synced',
      paymentStatus: 'Double Debit Detected (Tx1: BK-912, Tx2: BK-913)',
      webrtcDiagnostics: {
        iceConnectionState: 'Completed (Clean Disconnect)',
        audioQuality: '100% Quality',
        videoQuality: '720p HD',
        packetLossPercent: '0.2%',
        networkPingRtt: '45ms',
        jitter: '18ms',
        reconnectCount: 0
      },
      deviceInfo: 'iPhone 13 • iOS 17.4 • Wi-Fi',
      escrowStatus: 'Refund Disbursed (৳800)',
      auditTimeline: [
        { time: '17:59:12', event: 'bKash Payment Gateway timeout on Tx1 (৳800 debited)' },
        { time: '17:59:45', event: 'bKash Tx2 retry succeeded (৳800 debited again)' },
        { time: '18:00:00', event: 'Video consultation started' },
        { time: '18:10:00', event: 'Consultation concluded normally; Rx #044 synced' },
        { time: '18:10:44', event: 'Patient reported double debit grievance' }
      ]
    },
    status: 'REFUNDED',
    adjudication: {
      action: 'REFUNDED',
      date: 'Yesterday, 19:05:12',
      admin: 'SuperAdmin (Central Finance)',
      notes: 'Duplicate ৳800 refunded back to patient bKash wallet. Escrow TxID: BK-REF-91823.'
    }
  }
];

export interface TransactionItem {
  txId: string;
  gatewayRef: string;
  timestamp: string;
  consultationId: string;
  sessionType: string;
  type: 'INFLOW' | 'ESCROW' | 'PAYOUT' | 'REFUND';
  typeLabel: string;
  gateway: 'bKash' | 'Nagad' | 'Card' | 'MFS Bulk API';
  gatewayLabel: string;
  patient: { name: string; phone: string; id: string; avatar: string; avatarBg: string };
  doctor: { name: string; bmdc: string; specialty: string; avatar: string; avatarBg: string };
  grossAmount: number;
  platformFeePercent: number;
  platformFeeAmount: number;
  netAmount: number;
  direction: 'INFLOW' | 'PAYOUT' | 'REFUND';
  escrowStatus: 'ESCROW_LOCKED' | 'SETTLED' | 'REFUNDED';
  escrowStatusLabel: string;
  statusBadgeCls: string;
  gatewayPayload: Record<string, any>;
  auditTrail: { time: string; event: string }[];
}

export const INITIAL_TRANSACTIONS: TransactionItem[] = [
  {
    txId: 'TXN-BK-94812',
    gatewayRef: 'BK-MER-88492019',
    timestamp: 'Today, 10:28 AM',
    consultationId: 'CONS-9481',
    sessionType: 'Video Consultation (10 min)',
    type: 'ESCROW',
    typeLabel: 'Consultation Inflow (Escrow)',
    gateway: 'bKash',
    gatewayLabel: 'bKash Merchant',
    patient: { name: 'Sarah Khan', phone: '+880 1711-234567', id: 'USR-8921', avatar: 'SK', avatarBg: '#2563EB' },
    doctor: { name: 'Dr. Sabrina Akter', bmdc: 'BMDC #45821', specialty: 'Internal Medicine', avatar: 'SA', avatarBg: '#059669' },
    grossAmount: 800,
    platformFeePercent: 15,
    platformFeeAmount: 120,
    netAmount: 680,
    direction: 'INFLOW',
    escrowStatus: 'ESCROW_LOCKED',
    escrowStatusLabel: 'Locked in Escrow 🔒',
    statusBadgeCls: 'badge-amber',
    gatewayPayload: {
      merchantInvoiceNumber: 'INV-2026-CONS-9481',
      paymentExecuteTime: '2026-09-30T10:28:14.218+06:00',
      payerReference: '01711234567',
      trxID: 'BK-MER-88492019',
      merchantAccount: '01713-HELODOC-MERCHANT',
      transactionStatus: 'Completed',
      signature: 'HMAC_SHA256_VERIFIED_7a9c1e'
    },
    auditTrail: [
      { time: '10:28:02', event: 'Payment initiated via bKash Checkout URL' },
      { time: '10:28:14', event: 'bKash Webhook Callback 200 OK (TrxID: BK-MER-88492019)' },
      { time: '10:28:15', event: 'HeloDoc Escrow Lock Activated (৳800 held in central vault)' },
      { time: '10:32:14', event: 'Video call ended prematurely (2m 14s). Grievance GRV-20260930-01 logged' },
      { time: '10:32:15', event: 'Automatic Escrow Disbursement Frozen pending Medical Board Adjudication' }
    ]
  },
  {
    txId: 'TXN-NG-77124',
    gatewayRef: 'NG-TXN-4921004',
    timestamp: 'Today, 09:45 AM',
    consultationId: 'CONS-9482',
    sessionType: 'Pediatric Surgery Review (15 min)',
    type: 'INFLOW',
    typeLabel: 'Consultation Inflow',
    gateway: 'Nagad',
    gatewayLabel: 'Nagad Business',
    patient: { name: 'Rafiq Ahmed', phone: '+880 1712-345678', id: 'USR-8922', avatar: 'RA', avatarBg: '#1D4ED8' },
    doctor: { name: 'Dr. Sadik Al-Amin', bmdc: 'BMDC #A-68192', specialty: 'Pediatric Surgery', avatar: 'SA', avatarBg: '#D97706' },
    grossAmount: 1200,
    platformFeePercent: 15,
    platformFeeAmount: 180,
    netAmount: 1020,
    direction: 'INFLOW',
    escrowStatus: 'SETTLED',
    escrowStatusLabel: 'Settled ✓',
    statusBadgeCls: 'badge-green',
    gatewayPayload: {
      merchantInvoiceNumber: 'INV-2026-CONS-9482',
      paymentExecuteTime: '2026-09-30T09:45:08.102+06:00',
      payerReference: '01712345678',
      trxID: 'NG-TXN-4921004',
      merchantAccount: '01844-HELODOC-NAGAD',
      transactionStatus: 'Completed',
      signature: 'HMAC_SHA256_VERIFIED_882b01'
    },
    auditTrail: [
      { time: '09:44:50', event: 'Nagad Direct Checkout token granted' },
      { time: '09:45:08', event: 'Payment confirmed by Nagad Webhook' },
      { time: '09:45:10', event: 'Held in Escrow during 15m consultation' },
      { time: '10:00:22', event: 'Consultation completed & signed off. Escrow released to physician wallet.' }
    ]
  },
  {
    txId: 'TXN-BK-88291',
    gatewayRef: 'BK-MER-3918402',
    timestamp: 'Today, 09:12 AM',
    consultationId: 'CHAT-4819',
    sessionType: '24h Continuous Chat Subscription',
    type: 'INFLOW',
    typeLabel: '24h Chat Subscription',
    gateway: 'bKash',
    gatewayLabel: 'bKash Merchant',
    patient: { name: 'Nusrat Jahan', phone: '+880 1819-456789', id: 'USR-8923', avatar: 'NJ', avatarBg: '#9333EA' },
    doctor: { name: 'Dr. Anika Tahsin', bmdc: 'BMDC #A-59124', specialty: 'Dermatology Consultant', avatar: 'AT', avatarBg: '#EC4899' },
    grossAmount: 300,
    platformFeePercent: 20,
    platformFeeAmount: 60,
    netAmount: 240,
    direction: 'INFLOW',
    escrowStatus: 'SETTLED',
    escrowStatusLabel: 'Settled ✓',
    statusBadgeCls: 'badge-green',
    gatewayPayload: {
      merchantInvoiceNumber: 'INV-2026-CHAT-4819',
      paymentExecuteTime: '2026-09-30T09:12:30.540+06:00',
      payerReference: '01819456789',
      trxID: 'BK-MER-3918402',
      merchantAccount: '01713-HELODOC-MERCHANT',
      transactionStatus: 'Completed',
      signature: 'HMAC_SHA256_VERIFIED_4f810c'
    },
    auditTrail: [
      { time: '09:12:10', event: 'Chat checkout triggered' },
      { time: '09:12:30', event: 'Instant bKash debit confirmed' },
      { time: '09:12:31', event: '24h timer initialized. 20% platform cut retained, ৳240 credited to doctor ledger.' }
    ]
  },
  {
    txId: 'TXN-RF-91823',
    gatewayRef: 'BK-REF-91823',
    timestamp: 'Today, 08:35 AM',
    consultationId: 'CONS-9390',
    sessionType: 'Escrow Dispute Refund',
    type: 'REFUND',
    typeLabel: 'Patient Grievance Refund',
    gateway: 'bKash',
    gatewayLabel: 'bKash Instant Refund',
    patient: { name: 'Tanvir Hasan', phone: '+880 1612-987654', id: 'USR-8924', avatar: 'TH', avatarBg: '#DC2626' },
    doctor: { name: 'Central Escrow Vault', bmdc: 'SYSTEM-REVERSAL', specialty: 'Platform Reconciliation', avatar: 'HD', avatarBg: '#0F172A' },
    grossAmount: 800,
    platformFeePercent: 0,
    platformFeeAmount: 0,
    netAmount: 800,
    direction: 'REFUND',
    escrowStatus: 'REFUNDED',
    escrowStatusLabel: 'Refunded ↩️',
    statusBadgeCls: 'badge-rose',
    gatewayPayload: {
      merchantInvoiceNumber: 'INV-REF-CONS-9390',
      paymentExecuteTime: '2026-09-30T08:35:19.004+06:00',
      payerReference: '01612987654',
      trxID: 'BK-REF-91823',
      merchantAccount: '01713-HELODOC-MERCHANT',
      transactionStatus: 'Refunded',
      signature: 'HMAC_SHA256_VERIFIED_99c31a'
    },
    auditTrail: [
      { time: '08:30:00', event: 'Patient lodged grievance for dropped call (38s duration)' },
      { time: '08:34:40', event: 'Medical Board confirmed premature disconnect; ordered 100% refund' },
      { time: '08:35:19', event: 'Instant bKash wallet reversal executed. TrxID: BK-REF-91823' }
    ]
  },
  {
    txId: 'TXN-CD-66120',
    gatewayRef: 'SSL-CARD-773194',
    timestamp: 'Today, 08:10 AM',
    consultationId: 'CONS-9388',
    sessionType: 'Video Visit (10 min)',
    type: 'INFLOW',
    typeLabel: 'Consultation Inflow',
    gateway: 'Card',
    gatewayLabel: 'Visa / Mastercard (SSLCommerz)',
    patient: { name: 'Kamal Hossain', phone: '+880 1912-345678', id: 'USR-8925', avatar: 'KH', avatarBg: '#047857' },
    doctor: { name: 'Dr. Farhana Yeasmin', bmdc: 'BMDC #A-71203', specialty: 'Obstetrics & Gynecology', avatar: 'FY', avatarBg: '#7C3AED' },
    grossAmount: 1000,
    platformFeePercent: 15,
    platformFeeAmount: 150,
    netAmount: 850,
    direction: 'INFLOW',
    escrowStatus: 'SETTLED',
    escrowStatusLabel: 'Settled ✓',
    statusBadgeCls: 'badge-green',
    gatewayPayload: {
      merchantInvoiceNumber: 'INV-2026-CONS-9388',
      paymentExecuteTime: '2026-09-30T08:10:44.811+06:00',
      payerReference: 'CARD-411122******1234',
      trxID: 'SSL-CARD-773194',
      merchantAccount: 'SSLCOMMERZ-HELODOC-LIVE',
      transactionStatus: 'VALIDATED',
      signature: 'SSL_HASH_VERIFIED_331b2'
    },
    auditTrail: [
      { time: '08:10:02', event: 'SSLCommerz payment gateway validated' },
      { time: '08:10:44', event: 'IPN Webhook received & 3D-Secure verified' },
      { time: '08:25:00', event: 'Consultation completed. ৳850 settled to Dr. Farhana wallet.' }
    ]
  },
  {
    txId: 'TXN-PO-99411',
    gatewayRef: 'BULK-MFS-99120',
    timestamp: 'Yesterday, 11:59 PM',
    consultationId: 'BATCH-PO-20260929',
    sessionType: 'Physician Wallet Daily Settlement',
    type: 'PAYOUT',
    typeLabel: 'Doctor Batch Payout',
    gateway: 'MFS Bulk API',
    gatewayLabel: 'bKash + Nagad Bulk API',
    patient: { name: 'HeloDoc Treasury', phone: 'SYSTEM', id: 'TREASURY-01', avatar: 'HD', avatarBg: '#0F172A' },
    doctor: { name: 'Active Physicians (18 Doctors)', bmdc: 'MULTI-RECON', specialty: 'Batch Settlement', avatar: 'DR', avatarBg: '#2563EB' },
    grossAmount: 156825,
    platformFeePercent: 0,
    platformFeeAmount: 0,
    netAmount: 156825,
    direction: 'PAYOUT',
    escrowStatus: 'SETTLED',
    escrowStatusLabel: 'Disbursed ✓',
    statusBadgeCls: 'badge-green',
    gatewayPayload: {
      merchantInvoiceNumber: 'BATCH-SETTLE-20260929',
      paymentExecuteTime: '2026-09-29T23:59:00.000+06:00',
      payerReference: 'HELODOC-TREASURY',
      trxID: 'BULK-MFS-99120',
      merchantAccount: 'MFS-DISBURSEMENT-CORP',
      transactionStatus: 'PROCESSED',
      signature: 'MFS_BULK_HASH_991823'
    },
    auditTrail: [
      { time: '23:58:30', event: 'Daily batch settlement compiled for 18 eligible doctors' },
      { time: '23:59:00', event: 'Disbursement sent via bKash & Nagad Corporate APIs' },
      { time: '23:59:12', event: 'All 18 physician wallets credited. Escrow ledger cleared.' }
    ]
  },
  {
    txId: 'TXN-BK-94815',
    gatewayRef: 'BK-MER-7718290',
    timestamp: 'Yesterday, 06:20 PM',
    consultationId: 'CONS-9340',
    sessionType: 'Video Consultation (10 min)',
    type: 'INFLOW',
    typeLabel: 'Consultation Inflow',
    gateway: 'bKash',
    gatewayLabel: 'bKash Merchant',
    patient: { name: 'Farzana Haque', phone: '+880 1714-567890', id: 'USR-8926', avatar: 'FH', avatarBg: '#0891B2' },
    doctor: { name: 'Dr. Sabrina Akter', bmdc: 'BMDC #45821', specialty: 'Internal Medicine', avatar: 'SA', avatarBg: '#059669' },
    grossAmount: 800,
    platformFeePercent: 15,
    platformFeeAmount: 120,
    netAmount: 680,
    direction: 'INFLOW',
    escrowStatus: 'SETTLED',
    escrowStatusLabel: 'Settled ✓',
    statusBadgeCls: 'badge-green',
    gatewayPayload: {
      merchantInvoiceNumber: 'INV-2026-CONS-9340',
      paymentExecuteTime: '2026-09-29T18:20:12.441+06:00',
      payerReference: '01714567890',
      trxID: 'BK-MER-7718290',
      merchantAccount: '01713-HELODOC-MERCHANT',
      transactionStatus: 'Completed',
      signature: 'HMAC_SHA256_VERIFIED_11a84c'
    },
    auditTrail: [
      { time: '18:20:00', event: 'bKash Merchant Pay verified' },
      { time: '18:20:12', event: 'Funds held in Escrow' },
      { time: '18:31:00', event: '10m call concluded normally with e-Rx #Rx-084 issued. Escrow settled.' }
    ]
  },
  {
    txId: 'TXN-NG-55201',
    gatewayRef: 'NG-TXN-102948',
    timestamp: 'Yesterday, 04:15 PM',
    consultationId: 'CONS-9321',
    sessionType: 'General Health Consult (10 min)',
    type: 'INFLOW',
    typeLabel: 'Consultation Inflow',
    gateway: 'Nagad',
    gatewayLabel: 'Nagad Business',
    patient: { name: 'Sarah Ahmed', phone: '+880 1711-234567', id: 'USR-6521', avatar: 'SA', avatarBg: '#BE185D' },
    doctor: { name: 'Dr. Kamal Uddin', bmdc: 'BMDC #A-39182', specialty: 'General Practice', avatar: 'KU', avatarBg: '#059669' },
    grossAmount: 600,
    platformFeePercent: 15,
    platformFeeAmount: 90,
    netAmount: 510,
    direction: 'INFLOW',
    escrowStatus: 'SETTLED',
    escrowStatusLabel: 'Settled ✓',
    statusBadgeCls: 'badge-green',
    gatewayPayload: {
      merchantInvoiceNumber: 'INV-2026-CONS-9321',
      paymentExecuteTime: '2026-09-29T16:15:33.201+06:00',
      payerReference: '01711234567',
      trxID: 'NG-TXN-102948',
      merchantAccount: '01844-HELODOC-NAGAD',
      transactionStatus: 'Completed',
      signature: 'HMAC_SHA256_VERIFIED_08c44e'
    },
    auditTrail: [
      { time: '16:15:10', event: 'Nagad Business token executed' },
      { time: '16:15:33', event: 'Escrow lock active' },
      { time: '16:26:00', event: '10m consultation completed. ৳510 net paid to Dr. Kamal.' }
    ]
  }
];
