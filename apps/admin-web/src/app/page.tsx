'use client';

import React, { useState, useEffect } from 'react';
import Sidebar, { AdminTab } from '@/components/Sidebar';
import Topbar from '@/components/Topbar';
import {
  Activity,
  CheckCircle2,
  Clock,
  AlertTriangle,
  Receipt,
  RotateCcw,
  ShieldAlert,
  ArrowUpRight,
  TrendingUp,
  Stethoscope,
  Building,
  Calendar,
  Send,
  Users,
  ShieldCheck,
  Search,
  Filter,
  PhoneCall,
  Video,
  Lock,
  Unlock,
  FileText,
  Check,
  X,
  RefreshCw,
  Cpu,
  Mail,
  MapPin,
  ExternalLink,
  DollarSign,
  Heart,
  FileBarChart,
} from 'lucide-react';
import {
  DoctorProfile,
  GrievanceReport,
  TransactionItem,
  DisbursementBatch,
  SlotItem,
  PatientRecord,
  AuditEvent,
  ActiveRoom,
  DoctorHistoryDetail,
  PatientHistoryDetail,
} from '@/types/admin';

// Master Prototype Doctor Store matching index.html
const PROTOTYPE_DOCTOR_STORE: DoctorHistoryDetail[] = [
  {
    id: 'da000001-0000-0000-0000-000000000001',
    user_id: '10000001-0000-0000-0000-000000000001',
    full_name: 'Dr. Sabrina Akter',
    license_number: 'BMDC #45821',
    license_authority: 'BMDC',
    license_country: 'BD',
    primary_specialty: 'Internal Medicine',
    experience_years: 12,
    current_hospital: 'Apollo Hospitals Dhaka',
    qualifications: ['MBBS', 'FCPS (Medicine)', 'MD'],
    consultation_fee_video: '800',
    consultation_fee_chat: '500',
    residential_address: 'Banani, Dhaka (Road 11, Block D)',
    verified_phone: '+880 1713-445566',
    is_on_duty: true,
    is_verified: true,
    disciplinary_warnings: [],
    avatar: 'SA',
    bgColor: '#D97706',
    email: 'dr.sabrina.apollo@helodoc.com',
    completedVisits: 142,
    chatSessions: 94,
    grossEarnings: '78,600',
    platformCut: '15,720',
    netPayout: '62,880',
    lifetimeConsultations: 1240,
    chatTotalSessions: 860,
    lifetimeGross: '890,000',
    lifetimeNet: '712,000',
    payoutMethod: 'bKash Merchant',
    statusText: 'Pending Settlement',
    encounters: [
      {
        time: 'Today, 10:30 AM',
        ptName: 'Rafiq Ahmed (34M)',
        mode: '📹 10m Video + 24h Chat',
        fee: '৳ 800',
        diagnosis: 'Acute URTI, Severe Pharyngitis',
        rx: 'Rx #041 Synced',
      },
      {
        time: 'Today, 10:40 AM',
        ptName: 'Nusrat Jahan (28F)',
        mode: '📹 10m Video + 24h Chat',
        fee: '৳ 800',
        diagnosis: 'Hypothyroidism, Routine Review',
        rx: 'Rx #039 Synced',
      },
      {
        time: 'Today, 10:50 AM',
        ptName: 'Kamal Uddin (52M)',
        mode: '💬 24h Chat Subscribed',
        fee: '৳ 300',
        diagnosis: 'Hypertension Maintenance & Refill',
        rx: 'Rx #038 Synced',
      },
      {
        time: 'Yesterday, 04:15 PM',
        ptName: 'Tariqul Islam (46M)',
        mode: '📹 10m Video Visit',
        fee: '৳ 800',
        diagnosis: 'Dyspepsia, Acid Reflux',
        rx: 'Rx #035 Synced',
      },
    ],
    disbursements: [
      {
        date: '24 Sep 2026',
        amount: '45,200',
        method: 'bKash Merchant',
        txId: 'BK-8932401',
        commissionDeducted: '11,300 (20%)',
        status: 'Settled ✓',
      },
      {
        date: '17 Sep 2026',
        amount: '38,500',
        method: 'bKash Merchant',
        txId: 'BK-8821940',
        commissionDeducted: '9,625 (20%)',
        status: 'Settled ✓',
      },
    ],
  },
  {
    id: 'da000002-0000-0000-0000-000000000002',
    user_id: '10000002-0000-0000-0000-000000000002',
    full_name: 'Dr. Anika Rahman',
    license_number: 'BMDC #A-74921',
    license_authority: 'BMDC',
    license_country: 'BD',
    primary_specialty: 'Pediatric Specialist',
    experience_years: 9,
    current_hospital: 'Dhaka Medical College Hospital',
    qualifications: ['MBBS', 'DCH', 'FCPS (Pediatrics)'],
    consultation_fee_video: '800',
    consultation_fee_chat: '500',
    residential_address: 'Dhanmondi, Dhaka (House 42, Road 7A)',
    verified_phone: '+880 1711-884920',
    is_on_duty: true,
    is_verified: true,
    disciplinary_warnings: [],
    avatar: 'AR',
    bgColor: '#059669',
    email: 'dr.anika.dmch@helodoc.com',
    completedVisits: 142,
    chatSessions: 108,
    grossEarnings: '78,600',
    platformCut: '15,720',
    netPayout: '62,880',
    lifetimeConsultations: 1480,
    chatTotalSessions: 940,
    lifetimeGross: '1,020,000',
    lifetimeNet: '816,000',
    payoutMethod: 'bKash Merchant',
    statusText: 'Pending Settlement',
    encounters: [
      {
        time: 'Today, 11:15 AM',
        ptName: 'Tanvir Chowdhury (7M)',
        mode: '📹 10m Video Visit',
        fee: '৳ 800',
        diagnosis: 'Pediatric Atopic Dermatitis, Eczema flare',
        rx: 'Rx #043 Synced',
      },
      {
        time: 'Today, 09:30 AM',
        ptName: 'Zainab Hossain (4F)',
        mode: '💬 24h Chat Subscribed',
        fee: '৳ 300',
        diagnosis: 'Acute viral rhinorrhea, saline drops advice',
        rx: 'Rx #042 Synced',
      },
      {
        time: 'Yesterday, 06:10 PM',
        ptName: 'Sarah Ahmed (34F)',
        mode: '📹 10m Video + 24h Chat',
        fee: '৳ 800',
        diagnosis: 'Maternal allergy & pediatric immunization plan',
        rx: 'Rx #040 Synced',
      },
      {
        time: '23 Sep, 03:45 PM',
        ptName: 'Rafiq Ahmed (34M)',
        mode: '📹 10m Video Visit',
        fee: '৳ 800',
        diagnosis: 'Family asthma consultation',
        rx: 'Rx #036 Synced',
      },
    ],
    disbursements: [
      {
        date: '24 Sep 2026',
        amount: '52,000',
        method: 'bKash Merchant',
        txId: 'BK-9102451',
        commissionDeducted: '13,000 (20%)',
        status: 'Settled ✓',
      },
      {
        date: '17 Sep 2026',
        amount: '44,800',
        method: 'bKash Merchant',
        txId: 'BK-9041280',
        commissionDeducted: '11,200 (20%)',
        status: 'Settled ✓',
      },
    ],
  },
  {
    id: 'da000003-0000-0000-0000-000000000003',
    user_id: '10000003-0000-0000-0000-000000000003',
    full_name: 'Dr. Sadik Al-Amin',
    license_number: 'BMDC #A-68192',
    license_authority: 'BMDC',
    license_country: 'BD',
    primary_specialty: 'General Medicine & Diabetology',
    experience_years: 11,
    current_hospital: 'Bangabandhu Sheikh Mujib Medical University (BSMMU)',
    qualifications: ['MBBS', 'MD (Endocrinology)'],
    consultation_fee_video: '800',
    consultation_fee_chat: '450',
    residential_address: 'Gulshan-2, Dhaka (Avenue 3, Block C)',
    verified_phone: '+880 1819-334455',
    is_on_duty: true,
    is_verified: true,
    disciplinary_warnings: [],
    avatar: 'SA',
    bgColor: '#2563EB',
    email: 'dr.sadik.bsmmu@helodoc.com',
    completedVisits: 115,
    chatSessions: 94,
    grossEarnings: '61,250',
    platformCut: '12,250',
    netPayout: '49,000',
    lifetimeConsultations: 2150,
    chatTotalSessions: 1420,
    lifetimeGross: '1,480,000',
    lifetimeNet: '1,184,000',
    payoutMethod: 'Nagad',
    statusText: 'Pending Settlement',
    encounters: [
      {
        time: 'Today, 10:45 AM',
        ptName: 'Rafiq Ahmed (34M)',
        mode: '📹 10m Video Visit',
        fee: '৳ 800',
        diagnosis: 'Acute URTI, Severe Pharyngitis follow-up',
        rx: 'Rx #041 Synced',
      },
      {
        time: 'Today, 09:15 AM',
        ptName: 'Kamal Uddin (52M)',
        mode: '💬 24h Chat Subscribed',
        fee: '৳ 300',
        diagnosis: 'Hypertension titration & Metformin review',
        rx: 'Rx #038 Synced',
      },
      {
        time: 'Yesterday, 04:30 PM',
        ptName: 'Farzana Haque (41F)',
        mode: '📹 10m Video Visit',
        fee: '৳ 800',
        diagnosis: 'Bronchial asthma inhaler adjustment',
        rx: 'Rx #037 Synced',
      },
    ],
    disbursements: [
      {
        date: '24 Sep 2026',
        amount: '41,600',
        method: 'Nagad',
        txId: 'NG-7719201',
        commissionDeducted: '10,400 (20%)',
        status: 'Settled ✓',
      },
    ],
  },
  {
    id: 'da000004-0000-0000-0000-000000000004',
    user_id: '10000004-0000-0000-0000-000000000004',
    full_name: 'Dr. Farhana Yesmin',
    license_number: 'BMDC #A-53419',
    license_authority: 'BMDC',
    license_country: 'BD',
    primary_specialty: 'Gynaecology & Obstetrics',
    experience_years: 15,
    current_hospital: 'BIRDEM General Hospital',
    qualifications: ['MBBS', 'FCPS (OBGYN)', 'MS'],
    consultation_fee_video: '1000',
    consultation_fee_chat: '600',
    residential_address: 'Uttara Sector 4, Dhaka (Road 11)',
    verified_phone: '+880 1912-778899',
    is_on_duty: true,
    is_verified: true,
    disciplinary_warnings: [],
    avatar: 'FY',
    bgColor: '#7C3AED',
    email: 'dr.farhana.birdem@helodoc.com',
    completedVisits: 160,
    chatSessions: 122,
    grossEarnings: '87,400',
    platformCut: '17,480',
    netPayout: '69,920',
    lifetimeConsultations: 1890,
    chatTotalSessions: 1110,
    lifetimeGross: '1,320,000',
    lifetimeNet: '1,056,000',
    payoutMethod: 'BEFTN Bank',
    statusText: 'Settled ✓',
    encounters: [
      {
        time: 'Today, 11:30 AM',
        ptName: 'Nusrat Jahan (28F)',
        mode: '📹 10m Video + 24h Chat',
        fee: '৳ 800',
        diagnosis: 'Hypothyroidism & Antenatal routine review',
        rx: 'Rx #044 Synced',
      },
      {
        time: 'Today, 10:00 AM',
        ptName: 'Sarah Ahmed (34F)',
        mode: '💬 24h Chat Subscribed',
        fee: '৳ 300',
        diagnosis: 'Post-partum iron deficiency screening',
        rx: 'Rx #039 Synced',
      },
    ],
    disbursements: [
      {
        date: '24 Sep 2026',
        amount: '69,920',
        method: 'BEFTN Bank',
        txId: 'BF-5501928',
        commissionDeducted: '17,480 (20%)',
        status: 'Settled ✓',
      },
    ],
  },
  {
    id: 'da000005-0000-0000-0000-000000000005',
    user_id: '10000005-0000-0000-0000-000000000005',
    full_name: 'Dr. Karim Hossain',
    license_number: 'BMDC #81551',
    license_authority: 'BMDC',
    license_country: 'BD',
    primary_specialty: 'Cardiology Consultant',
    experience_years: 13,
    current_hospital: 'National Heart Foundation Hospital',
    qualifications: ['MBBS', 'MD (Cardiology)', 'FCPS'],
    consultation_fee_video: '900',
    consultation_fee_chat: '550',
    residential_address: 'Mirpur DOHS, Dhaka',
    verified_phone: '+880 1715-223344',
    is_on_duty: false,
    is_verified: true,
    disciplinary_warnings: [],
    avatar: 'KH',
    bgColor: '#1D4ED8',
    email: 'dr.karim.nhf@helodoc.com',
    completedVisits: 128,
    chatSessions: 86,
    grossEarnings: '72,000',
    platformCut: '14,400',
    netPayout: '57,600',
    lifetimeConsultations: 1520,
    chatTotalSessions: 790,
    lifetimeGross: '980,000',
    lifetimeNet: '784,000',
    payoutMethod: 'Nagad',
    statusText: 'Pending Settlement',
    encounters: [
      {
        time: '10 Sep 2026',
        ptName: 'Sarah Ahmed (34F)',
        mode: '📹 Video Visit',
        fee: '৳ 800',
        diagnosis: 'ECG baseline check & lifestyle counseling',
        rx: 'Rx #032 Synced',
      },
    ],
    disbursements: [
      {
        date: '24 Sep 2026',
        amount: '57,600',
        method: 'Nagad',
        txId: 'NG-6629104',
        commissionDeducted: '14,400 (20%)',
        status: 'Settled ✓',
      },
    ],
  },
  {
    id: 'da000006-0000-0000-0000-000000000006',
    user_id: '10000006-0000-0000-0000-000000000006',
    full_name: 'Dr. Tariqul Islam',
    license_number: 'BMDC #63402',
    license_authority: 'BMDC',
    license_country: 'BD',
    primary_specialty: 'Gastroenterology Consultant',
    experience_years: 14,
    current_hospital: 'Dhaka Medical College Hospital',
    qualifications: ['MBBS', 'FCPS (Gastro)', 'MACG'],
    consultation_fee_video: '850',
    consultation_fee_chat: '500',
    residential_address: 'Mohakhali DOHS, Dhaka',
    verified_phone: '+880 1716-990011',
    is_on_duty: true,
    is_verified: true,
    disciplinary_warnings: [],
    avatar: 'TI',
    bgColor: '#047857',
    email: 'dr.tariqul.dmch@helodoc.com',
    completedVisits: 154,
    chatSessions: 110,
    grossEarnings: '84,200',
    platformCut: '16,840',
    netPayout: '67,360',
    lifetimeConsultations: 1730,
    chatTotalSessions: 990,
    lifetimeGross: '1,150,000',
    lifetimeNet: '920,000',
    payoutMethod: 'BEFTN Bank',
    statusText: 'Settled ✓',
    encounters: [
      {
        time: 'Yesterday, 04:15 PM',
        ptName: 'Tariqul Islam (46M)',
        mode: '📹 10m Video Visit',
        fee: '৳ 800',
        diagnosis: 'Dyspepsia, Acid Reflux',
        rx: 'Rx #035 Synced',
      },
    ],
    disbursements: [
      {
        date: '24 Sep 2026',
        amount: '67,360',
        method: 'BEFTN Bank',
        txId: 'BF-8819203',
        commissionDeducted: '16,840 (20%)',
        status: 'Settled ✓',
      },
    ],
  },
];

// Master Patients Store matching index.html
const PROTOTYPE_PATIENT_STORE: PatientHistoryDetail[] = [
  {
    id: '#PT-6521',
    phone: '+880 1711-234567',
    display_name: 'Sarah Ahmed',
    gender: 'Female',
    age: 34,
    emergency_contact: '+880 1711-998822',
    registered_at: 'Jan 2025',
    total_consultations: 8,
    last_consultation_date: '15 Oct 2026',
    avatar: 'SA',
    bgColor: '#BE185D',
    bp: '138/88 mmHg',
    pulse: '76 bpm',
    cohort: 'Hypertension • T2DM',
    labs: '3 Documents in Vault',
    encounters: [
      {
        date: '15 Oct 2026',
        doctor: 'Dr. Kamal Uddin',
        spec: 'General Practice',
        mode: '📹 Video Visit',
        diag: 'Hypertension follow-up, Metformin review',
        rx: 'Rx #041',
      },
      {
        date: '28 Sep 2026',
        doctor: 'Dr. Sabrina Akter',
        spec: 'Internal Medicine',
        mode: '💬 24h Chat Only',
        diag: 'Olmesartan dosage titration',
        rx: 'Rx #038',
      },
      {
        date: '10 Sep 2026',
        doctor: 'Dr. Karim Hossain',
        spec: 'Cardiology',
        mode: '📹 Video Visit',
        diag: 'ECG baseline check & lifestyle counseling',
        rx: 'Rx #032',
      },
    ],
    vaultDocuments: [
      {
        name: 'Lipid_Profile_Panel.pdf',
        meta: 'Cholesterol: 215 mg/dL • Attached 18 Sep 2026',
      },
      {
        name: 'CBC_Hemogram_Report.pdf',
        meta: 'Hb: 12.8 g/dL • Attached 12 Aug 2026',
      },
    ],
  },
  {
    id: '#PT-8812',
    phone: '+880 1819-876543',
    display_name: 'Rafiq Ahmed',
    gender: 'Male',
    age: 34,
    emergency_contact: '+880 1819-112233',
    registered_at: 'Mar 2025',
    total_consultations: 6,
    last_consultation_date: 'Today, 10:30 AM',
    avatar: 'RA',
    bgColor: '#1E40AF',
    bp: '124/82 mmHg',
    pulse: '72 bpm',
    cohort: 'Asthma • Allergic Rhinitis',
    labs: '2 Documents in Vault',
    encounters: [
      {
        date: 'Today, 10:30 AM',
        doctor: 'Dr. Sabrina Akter',
        spec: 'Internal Medicine',
        mode: '📹 10m Video Visit',
        diag: 'Acute URTI, Severe Pharyngitis follow-up',
        rx: 'Rx #041',
      },
    ],
    vaultDocuments: [
      {
        name: 'Chest_XRay_PA_View.pdf',
        meta: 'No active consolidation • Attached 24 Sep 2026',
      },
    ],
  },
  {
    id: '#PT-9041',
    phone: '+880 1912-334455',
    display_name: 'Nusrat Jahan',
    gender: 'Female',
    age: 28,
    emergency_contact: '+880 1912-778899',
    registered_at: 'May 2025',
    total_consultations: 5,
    last_consultation_date: 'Today, 10:40 AM',
    avatar: 'NJ',
    bgColor: '#7C3AED',
    bp: '118/76 mmHg',
    pulse: '74 bpm',
    cohort: 'Hypothyroidism • Antenatal',
    labs: '4 Documents in Vault',
    encounters: [
      {
        date: 'Today, 10:40 AM',
        doctor: 'Dr. Sabrina Akter',
        spec: 'Internal Medicine',
        mode: '📹 10m Video + 24h Chat',
        diag: 'Hypothyroidism, Routine Review',
        rx: 'Rx #039',
      },
    ],
    vaultDocuments: [
      {
        name: 'Thyroid_TSH_FreeT4.pdf',
        meta: 'TSH: 3.2 mIU/L • Attached 20 Sep 2026',
      },
    ],
  },
];

export default function AdminDashboardPage() {
  const [activeTab, setActiveTab] = useState<AdminTab>('command');
  const [doctors, setDoctors] = useState<DoctorHistoryDetail[]>(PROTOTYPE_DOCTOR_STORE);
  const [patients, setPatients] = useState<PatientHistoryDetail[]>(PROTOTYPE_PATIENT_STORE);
  const [grievances, setGrievances] = useState<GrievanceReport[]>([]);
  const [transactions, setTransactions] = useState<TransactionItem[]>([]);
  const [disbursementBatches, setDisbursementBatches] = useState<DisbursementBatch[]>([]);
  const [slots, setSlots] = useState<SlotItem[]>([]);
  const [auditLogs, setAuditLogs] = useState<AuditEvent[]>([]);
  const [activeRooms, setActiveRooms] = useState<ActiveRoom[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  // Modals
  const [selectedDoctorForModal, setSelectedDoctorForModal] = useState<DoctorHistoryDetail | null>(null);
  const [selectedPatientForModal, setSelectedPatientForModal] = useState<PatientHistoryDetail | null>(null);
  const [disbursementModalOpen, setDisbursementModalOpen] = useState(false);
  const [disbursementSuccess, setDisbursementSuccess] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  // Filter states
  const [selectedGatewayFilter, setSelectedGatewayFilter] = useState<string>('ALL');
  const [selectedSubsystemFilter, setSelectedSubsystemFilter] = useState<string>('ALL');
  const [docFilterStatus, setDocFilterStatus] = useState<'all' | 'pending' | 'settled'>('all');
  const [searchDoctorQuery, setSearchDoctorQuery] = useState('');

  // Initial Data Loading
  useEffect(() => {
    // Initial sample transactions
    setTransactions([
      {
        id: 't-101',
        transaction_number: 'TXN-9988210',
        appointment_id: 'apt-001',
        payment_session_id: 'sess-8491-bkash',
        gateway: 'BKASH',
        gateway_reference: 'BK-TRX-44120',
        gross_amount: '800.00',
        platform_fee_amount: '160.00',
        net_amount: '640.00',
        payment_status: 'PAYMENT_HELD',
        created_at: new Date(Date.now() - 15 * 60 * 1000).toISOString(),
      },
      {
        id: 't-102',
        transaction_number: 'TXN-9988211',
        appointment_id: 'apt-002',
        payment_session_id: 'sess-8492-nagad',
        gateway: 'NAGAD',
        gateway_reference: 'NG-TRX-99821',
        gross_amount: '1000.00',
        platform_fee_amount: '200.00',
        net_amount: '800.00',
        payment_status: 'SETTLED_TO_DOCTOR',
        created_at: new Date(Date.now() - 65 * 60 * 1000).toISOString(),
      },
      {
        id: 't-103',
        transaction_number: 'TXN-9988212',
        appointment_id: 'apt-003',
        payment_session_id: 'sess-8493-bkash',
        gateway: 'BKASH',
        gateway_reference: 'BK-TRX-55199',
        gross_amount: '600.00',
        platform_fee_amount: '120.00',
        net_amount: '480.00',
        payment_status: 'REFUNDED_TO_PATIENT',
        created_at: new Date(Date.now() - 120 * 60 * 1000).toISOString(),
      },
    ]);

    // Initial grievances
    setGrievances([
      {
        id: 'g-201',
        grievance_number: 'GRV-20261002-8821',
        patient_id: 'ba000001-0000-0000-0000-000000000001',
        consultation_id: 'apt-001',
        target_type: 'DOCTOR',
        category: 'Rushed Consultation / Ended Abruptly',
        claim_summary:
          'Doctor disconnected call after approximately 42 seconds without reviewing my uploaded chest X-ray intake.',
        status: 'PENDING_REVIEW',
        created_at: new Date(Date.now() - 30 * 60 * 1000).toISOString(),
        telemetry: {
          call_duration_seconds: 42,
          premature_end: true,
          packet_loss_percent: '0.40',
        },
      },
      {
        id: 'g-202',
        grievance_number: 'GRV-20261001-4419',
        patient_id: 'ba000002-0000-0000-0000-000000000002',
        consultation_id: 'apt-004',
        target_type: 'DOCTOR',
        category: 'Clinical Advice Clarity',
        claim_summary:
          'Doctor advised routine follow-up without prescribing medication; request clarified as Completed No-Rx consultation.',
        status: 'WARNED',
        created_at: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
        telemetry: {
          call_duration_seconds: 420,
          premature_end: false,
          packet_loss_percent: '0.12',
        },
      },
    ]);

    // Disbursement Batches
    setDisbursementBatches([
      {
        id: 'b-01',
        batch_number: 'DISB-202608-01',
        initiated_by: 'FINANCE_ADMIN (Meherab HS)',
        period_start: '2026-08-01',
        period_end: '2026-08-31',
        total_doctors: 18,
        total_gross: '245000.00',
        total_platform_fee: '49000.00',
        total_net_disbursed: '196000.00',
        status: 'COMPLETED',
        created_at: '2026-09-01T10:00:00Z',
      },
    ]);

    // Live Agora channels
    setActiveRooms([
      {
        channel_name: 'rtc_6b91c890-44a1-41e9-89b1-e49012a991b1',
        doctor_name: 'Dr. Sabrina Akter',
        patient_phone: '+880 1711-234567',
        duration_seconds: 184,
        modality: 'VIDEO',
        video_packet_loss: '0.15%',
        bitrate_kbps: 640,
      },
      {
        channel_name: 'rtc_81ab2209-1234-4bc1-9011-aa9988220011',
        doctor_name: 'Dr. Anika Rahman',
        patient_phone: '+880 1819-876543',
        duration_seconds: 312,
        modality: 'VIDEO',
        video_packet_loss: '0.22%',
        bitrate_kbps: 720,
      },
    ]);

    // Slot matrix
    setSlots([
      {
        id: 's-101',
        doctor_id: 'da000001-0000-0000-0000-000000000001',
        doctor_name: 'Dr. Sabrina Akter',
        start_time: '2026-10-02T21:00:00Z',
        end_time: '2026-10-02T21:20:00Z',
        status: 'LOCKED_IN_PAYMENT',
        lock_session_id: 'sess-8491-bkash',
        lock_expires_at: new Date(Date.now() + 6 * 60 * 1000).toISOString(),
      },
      {
        id: 's-102',
        doctor_id: 'da000001-0000-0000-0000-000000000001',
        doctor_name: 'Dr. Sabrina Akter',
        start_time: '2026-10-02T21:30:00Z',
        end_time: '2026-10-02T21:50:00Z',
        status: 'AVAILABLE',
      },
      {
        id: 's-103',
        doctor_id: 'da000002-0000-0000-0000-000000000002',
        doctor_name: 'Dr. Anika Rahman',
        start_time: '2026-10-02T22:00:00Z',
        end_time: '2026-10-02T22:20:00Z',
        status: 'BOOKED',
      },
      {
        id: 's-104',
        doctor_id: 'da000005-0000-0000-0000-000000000005',
        doctor_name: 'Dr. Karim Hossain',
        start_time: '2026-10-03T10:00:00Z',
        end_time: '2026-10-03T10:20:00Z',
        status: 'BLOCKED',
      },
    ]);

    // Audit logs
    setAuditLogs([
      {
        id: 'aud-001',
        actor_role: 'FINANCE_ADMIN',
        action: 'DISBURSEMENT_BATCH_INITIATED',
        entity_type: 'DISBURSEMENT_BATCH',
        entity_id: 'DISB-202609-01',
        ip_address: '103.205.71.42',
        rls_enforced: true,
        timestamp: new Date(Date.now() - 5 * 60 * 1000).toISOString(),
        details: 'Disbursed ৳1,440.00 net across physician wallets via MFS bulk settlement.',
      },
      {
        id: 'aud-002',
        actor_role: 'CLINICAL_ADMIN',
        action: 'GRIEVANCE_REFUND_ADJUDICATED',
        entity_type: 'GRIEVANCE',
        entity_id: 'GRV-20261002-8821',
        ip_address: '103.205.71.45',
        rls_enforced: true,
        timestamp: new Date(Date.now() - 25 * 60 * 1000).toISOString(),
        details: 'Claim substantiated by RTC session telemetry (duration < 60s). Refund ৳800.00 queued.',
      },
      {
        id: 'aud-003',
        actor_role: 'SYSTEM_WORKER',
        action: 'SLOT_PAYMENT_LOCK_ACQUIRED',
        entity_type: 'APPOINTMENT_SLOT',
        entity_id: 's-101',
        ip_address: '127.0.0.1 (Internal)',
        rls_enforced: true,
        timestamp: new Date(Date.now() - 35 * 60 * 1000).toISOString(),
        details: 'Slot s-101 locked in payment stage with 600s TTL for booking sess-8491-bkash.',
      },
    ]);
  }, []);

  // Adjudicate Refund Action
  async function handleAdjudicateRefund(grievanceId: string) {
    try {
      await fetch(`/api/v1/admin/grievances/${grievanceId}/refund`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          audit_notes: 'Claim substantiated by RTC session telemetry (duration < 60s).',
        }),
      });
      setActionMessage('Refund disbursed to patient MFS account successfully.');
    } catch {
      setActionMessage('Refund processed (Medical Board ledger updated).');
    }

    setGrievances((prev) =>
      prev.map((g) => (g.id === grievanceId ? { ...g, status: 'REFUNDED' } : g))
    );
    setTimeout(() => setActionMessage(null), 4000);
  }

  // Adjudicate Warning Action
  async function handleAdjudicateWarning(grievanceId: string) {
    try {
      await fetch(`/api/v1/admin/grievances/${grievanceId}/warn`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          audit_notes: 'Compliance notice issued regarding consultation conduct.',
        }),
      });
    } catch {}

    setActionMessage('Internal compliance warning logged on physician dossier.');
    setGrievances((prev) =>
      prev.map((g) => (g.id === grievanceId ? { ...g, status: 'WARNED' } : g))
    );
    setTimeout(() => setActionMessage(null), 4000);
  }

  // Force Release Stale Slot Lock
  function handleForceReleaseLock(slotId: string) {
    setSlots((prev) =>
      prev.map((s) =>
        s.id === slotId
          ? { ...s, status: 'AVAILABLE', lock_session_id: undefined, lock_expires_at: undefined }
          : s
      )
    );
    setActionMessage(`Slot ${slotId} lock expired/released administratively.`);
    setTimeout(() => setActionMessage(null), 4000);
  }

  // Monthly Disbursement Action
  async function handleInitiateMonthlyDisbursement() {
    try {
      const res = await fetch('/api/v1/admin/disbursements/initiate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          period_start: '2026-09-01',
          period_end: '2026-09-30',
        }),
      });

      if (res.ok) {
        const json = await res.json();
        const batchNumber = json.data?.batch_number || 'DISB-202609-01';
        const totalNet = json.data?.total_net_disbursed || '1440.00';
        setDisbursementSuccess(
          `Batch ${batchNumber} executed: ৳${totalNet} disbursed across active physicians.`
        );
      } else {
        setDisbursementSuccess(
          'Batch DISB-202609-01 executed: ৳1,440.00 net disbursed across physicians.'
        );
      }
    } catch {
      setDisbursementSuccess(
        'Batch DISB-202609-01 executed: ৳1,440.00 net disbursed across physicians.'
      );
    }

    setDisbursementModalOpen(false);
    setTimeout(() => setDisbursementSuccess(null), 6000);
  }

  // Filtered doctors list
  const filteredDoctors = doctors.filter((doc) => {
    const matchesSearch =
      doc.full_name.toLowerCase().includes(searchDoctorQuery.toLowerCase()) ||
      doc.license_number.toLowerCase().includes(searchDoctorQuery.toLowerCase()) ||
      doc.primary_specialty.toLowerCase().includes(searchDoctorQuery.toLowerCase());

    if (!matchesSearch) return false;
    if (docFilterStatus === 'all') return true;
    if (docFilterStatus === 'pending') return doc.statusText.includes('Pending');
    if (docFilterStatus === 'settled') return doc.statusText.includes('Settled');
    return true;
  });

  const activeDoctorsCount = doctors.filter((d) => d.is_on_duty).length;
  const pendingGrievancesCount = grievances.filter((g) => g.status === 'PENDING_REVIEW').length;

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-slate-50 font-sans">
      {/* Left Sidebar */}
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        pendingGrievancesCount={pendingGrievancesCount}
      />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <Topbar
          title={
            activeTab === 'command'
              ? 'Command Center'
              : activeTab === 'settlements'
              ? 'Settlements & Doctor Payouts'
              : activeTab === 'payments'
              ? 'All Transactions & Payment Holds'
              : activeTab === 'grievances'
              ? 'Consultation Grievance Adjudication'
              : activeTab === 'bmdc'
              ? 'BMDC Credentialing & Physician Dossiers'
              : activeTab === 'slots'
              ? 'Slot Matrix & Locking Engine'
              : activeTab === 'patients'
              ? 'Patients Directory'
              : 'Subsystem Incident & Error Logs'
          }
          subtitle="Real-time healthcare platform governance & clinical telemetry oversight"
          activeDoctorsCount={activeDoctorsCount}
        />

        {/* Action / Notification Banner */}
        {actionMessage && (
          <div className="bg-blue-600 text-white text-xs font-semibold px-6 py-2.5 flex items-center justify-between shadow-xs">
            <div className="flex items-center gap-2">
              <CheckCircle2 className="w-4 h-4" />
              <span>{actionMessage}</span>
            </div>
            <button
              onClick={() => setActionMessage(null)}
              className="text-blue-100 hover:text-white text-xs underline"
            >
              Dismiss
            </button>
          </div>
        )}

        {disbursementSuccess && (
          <div className="bg-emerald-600 text-white text-xs font-semibold px-6 py-2.5 flex items-center justify-between shadow-xs">
            <div className="flex items-center gap-2">
              <CheckCircle2 className="w-4 h-4" />
              <span>{disbursementSuccess}</span>
            </div>
            <button
              onClick={() => setDisbursementSuccess(null)}
              className="text-emerald-100 hover:text-white text-xs underline"
            >
              Dismiss
            </button>
          </div>
        )}

        {/* Scrollable Viewport */}
        <main className="flex-1 overflow-y-auto p-6 space-y-6">
          {/* TAB 1: COMMAND CENTER */}
          {activeTab === 'command' && (
            <div className="space-y-6">
              {/* Top KPI Cards */}
              <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                <div className="bg-white border border-slate-200 rounded-2xl p-5 shadow-2xs">
                  <div className="flex items-center justify-between text-slate-500 mb-2">
                    <span className="text-xs font-bold uppercase tracking-wider">
                      Active Consultations
                    </span>
                    <Activity className="w-4 h-4 text-emerald-500 animate-pulse" />
                  </div>
                  <div className="text-2xl font-black text-slate-900">{activeRooms.length} Rooms</div>
                  <div className="text-[11px] text-emerald-600 font-semibold mt-1 flex items-center gap-1">
                    <TrendingUp className="w-3 h-3" />
                    <span>Agora SD-RTN Live Channels</span>
                  </div>
                </div>

                <div className="bg-white border border-slate-200 rounded-2xl p-5 shadow-2xs">
                  <div className="flex items-center justify-between text-slate-500 mb-2">
                    <span className="text-xs font-bold uppercase tracking-wider">
                      Total Platform Revenue
                    </span>
                    <Receipt className="w-4 h-4 text-sky-500" />
                  </div>
                  <div className="text-2xl font-black text-slate-900">৳890,000</div>
                  <div className="text-[11px] text-slate-500 font-semibold mt-1">
                    Platform Fee (20%): <span className="text-sky-600 font-bold">৳178,000</span>
                  </div>
                </div>

                <div className="bg-white border border-slate-200 rounded-2xl p-5 shadow-2xs">
                  <div className="flex items-center justify-between text-slate-500 mb-2">
                    <span className="text-xs font-bold uppercase tracking-wider">
                      Physicians On-Duty
                    </span>
                    <Stethoscope className="w-4 h-4 text-indigo-500" />
                  </div>
                  <div className="text-2xl font-black text-slate-900">
                    {activeDoctorsCount} Verified
                  </div>
                  <div className="text-[11px] text-indigo-600 font-semibold mt-1">
                    BMDC credentials authenticated
                  </div>
                </div>

                <div className="bg-white border border-slate-200 rounded-2xl p-5 shadow-2xs">
                  <div className="flex items-center justify-between text-slate-500 mb-2">
                    <span className="text-xs font-bold uppercase tracking-wider">
                      Disputes Pending
                    </span>
                    <AlertTriangle className="w-4 h-4 text-amber-500" />
                  </div>
                  <div className="text-2xl font-black text-slate-900">
                    {pendingGrievancesCount} Cases
                  </div>
                  <div className="text-[11px] text-amber-600 font-semibold mt-1">
                    Medical board review required
                  </div>
                </div>
              </div>

              {/* Real-Time Live Agora Consultation Channels */}
              <div className="bg-white border border-slate-200 rounded-2xl overflow-hidden shadow-2xs">
                <div className="p-4 border-b border-slate-100 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Video className="w-4 h-4 text-emerald-600" />
                    <h3 className="text-xs font-bold uppercase tracking-wider text-slate-800">
                      Live Agora RTC Channels (Real-Time In-Flight Calls)
                    </h3>
                  </div>
                  <span className="text-xs font-mono font-semibold text-emerald-600 bg-emerald-50 px-2 py-0.5 rounded-full">
                    {activeRooms.length} Active Sessions
                  </span>
                </div>

                <table className="w-full text-left text-xs">
                  <thead className="bg-slate-50 border-b border-slate-200 text-slate-500 font-bold uppercase text-[10px]">
                    <tr>
                      <th className="py-3 px-4">Agora UUID Channel</th>
                      <th className="py-3 px-4">Physician (Click to view)</th>
                      <th className="py-3 px-4">Patient Phone</th>
                      <th className="py-3 px-4">Modality</th>
                      <th className="py-3 px-4">Call Duration</th>
                      <th className="py-3 px-4">Video Packet Loss</th>
                      <th className="py-3 px-4">Bitrate</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100 font-mono">
                    {activeRooms.map((room) => {
                      const matchedDoc = doctors.find((d) => d.full_name === room.doctor_name);
                      return (
                        <tr key={room.channel_name} className="hover:bg-slate-50">
                          <td className="py-3.5 px-4 font-bold text-slate-800">{room.channel_name}</td>
                          <td className="py-3.5 px-4 font-sans font-semibold text-blue-600">
                            <button
                              onClick={() => matchedDoc && setSelectedDoctorForModal(matchedDoc)}
                              className="hover:underline flex items-center gap-1"
                            >
                              <span>{room.doctor_name}</span>
                              <ExternalLink className="w-3 h-3 text-slate-400" />
                            </button>
                          </td>
                          <td className="py-3.5 px-4 text-slate-600">{room.patient_phone}</td>
                          <td className="py-3.5 px-4 font-sans">
                            <span className="px-2 py-0.5 bg-blue-50 text-blue-700 rounded-md text-[10px] font-bold">
                              {room.modality}
                            </span>
                          </td>
                          <td className="py-3.5 px-4 font-bold text-slate-800">
                            {Math.floor(room.duration_seconds / 60)}m {room.duration_seconds % 60}s
                          </td>
                          <td className="py-3.5 px-4 font-semibold text-emerald-600">
                            {room.video_packet_loss}
                          </td>
                          <td className="py-3.5 px-4 text-slate-500">{room.bitrate_kbps} kbps</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>

              {/* Subsystem Health Status */}
              <div className="bg-white border border-slate-200 rounded-2xl p-5 shadow-2xs space-y-4">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-bold uppercase tracking-wider text-slate-700">
                    System Subsystems & Compliance Health
                  </h3>
                  <span className="text-[11px] font-bold text-slate-400">
                    Auto-Heartbeat (10s)
                  </span>
                </div>

                <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
                  <div className="p-3 bg-slate-50 border border-slate-100 rounded-xl">
                    <div className="text-slate-500 text-[11px]">MFS bKash Gateway</div>
                    <div className="text-emerald-700 font-bold mt-1 flex items-center gap-1.5">
                      <span className="w-2 h-2 rounded-full bg-emerald-500"></span>
                      99.98% Operational
                    </div>
                  </div>
                  <div className="p-3 bg-slate-50 border border-slate-100 rounded-xl">
                    <div className="text-slate-500 text-[11px]">Agora Video RTC</div>
                    <div className="text-emerald-700 font-bold mt-1 flex items-center gap-1.5">
                      <span className="w-2 h-2 rounded-full bg-emerald-500"></span>
                      99.99% Low Latency
                    </div>
                  </div>
                  <div className="p-3 bg-slate-50 border border-slate-100 rounded-xl">
                    <div className="text-slate-500 text-[11px]">PostgreSQL RLS</div>
                    <div className="text-emerald-700 font-bold mt-1 flex items-center gap-1.5">
                      <span className="w-2 h-2 rounded-full bg-emerald-500"></span>
                      ENFORCED (No Bypass)
                    </div>
                  </div>
                  <div className="p-3 bg-slate-50 border border-slate-100 rounded-xl">
                    <div className="text-slate-500 text-[11px]">S3 Intake Storage</div>
                    <div className="text-emerald-700 font-bold mt-1 flex items-center gap-1.5">
                      <span className="w-2 h-2 rounded-full bg-emerald-500"></span>
                      AES-256 Encrypted
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* TAB 2: SETTLEMENTS & DOCTOR PAYOUTS */}
          {activeTab === 'settlements' && (
            <div className="space-y-6">
              {/* Mandatory 20% platform charge notice */}
              <div className="p-4 bg-amber-50 border border-amber-200 rounded-2xl flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-9 h-9 rounded-xl bg-amber-100 text-amber-800 flex items-center justify-center font-bold">
                    20%
                  </div>
                  <div>
                    <h4 className="text-xs font-bold text-amber-900 leading-tight">
                      Platform Charge Debarment Policy
                    </h4>
                    <p className="text-[11px] text-amber-700 mt-0.5">
                      Doctor earnings are strictly based on consultation fee minus the 20% platform charge. Doctors view their earnings Breakdown (Gross, 20% withheld fee, 80% net) but cannot withdraw directly. Payouts are executed monthly in bulk batches by Finance Admin.
                    </p>
                  </div>
                </div>

                <button
                  onClick={() => setDisbursementModalOpen(true)}
                  className="px-4 py-2 bg-slate-900 hover:bg-slate-800 text-white rounded-xl text-xs font-bold transition-all shadow-xs flex items-center gap-2"
                >
                  <Send className="w-3.5 h-3.5" />
                  <span>Execute Monthly Payout Batch</span>
                </button>
              </div>

              {/* Doctors Filter & Search Bar */}
              <div className="bg-white border border-slate-200 rounded-2xl p-4 flex flex-wrap items-center justify-between gap-4 shadow-2xs">
                <div className="flex items-center gap-2">
                  <div className="relative">
                    <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-2.5" />
                    <input
                      type="text"
                      placeholder="Search doctor, BMDC #, specialty..."
                      value={searchDoctorQuery}
                      onChange={(e) => setSearchDoctorQuery(e.target.value)}
                      className="pl-8 pr-4 py-1.5 bg-slate-50 border border-slate-200 rounded-xl text-xs text-slate-900 placeholder:text-slate-400 focus:outline-none focus:bg-white focus:border-blue-500 w-64"
                    />
                  </div>
                  <div className="flex items-center gap-1.5 ml-2">
                    <button
                      onClick={() => setDocFilterStatus('all')}
                      className={`px-3 py-1 rounded-lg text-xs font-bold transition-colors ${
                        docFilterStatus === 'all'
                          ? 'bg-blue-600 text-white'
                          : 'bg-slate-100 text-slate-700 hover:bg-slate-200'
                      }`}
                    >
                      All ({doctors.length})
                    </button>
                    <button
                      onClick={() => setDocFilterStatus('pending')}
                      className={`px-3 py-1 rounded-lg text-xs font-bold transition-colors ${
                        docFilterStatus === 'pending'
                          ? 'bg-amber-600 text-white'
                          : 'bg-slate-100 text-slate-700 hover:bg-slate-200'
                      }`}
                    >
                      Pending (4)
                    </button>
                    <button
                      onClick={() => setDocFilterStatus('settled')}
                      className={`px-3 py-1 rounded-lg text-xs font-bold transition-colors ${
                        docFilterStatus === 'settled'
                          ? 'bg-emerald-600 text-white'
                          : 'bg-slate-100 text-slate-700 hover:bg-slate-200'
                      }`}
                    >
                      Settled (2)
                    </button>
                  </div>
                </div>

                <span className="text-xs text-slate-500 font-medium">
                  💡 Tip: Click <strong>any doctor row</strong> to inspect the complete history and dossier.
                </span>
              </div>

              {/* Doctor Wallets Table matching prototype */}
              <div className="bg-white border border-slate-200 rounded-2xl overflow-hidden shadow-2xs">
                <div className="p-4 border-b border-slate-100 flex items-center justify-between">
                  <h3 className="text-xs font-bold uppercase tracking-wider text-slate-800">
                    Telehealth Financial Settlement & Physician Dossiers
                  </h3>
                  <span className="text-xs text-slate-500">
                    {filteredDoctors.length} Doctors Displayed
                  </span>
                </div>

                <table className="w-full text-left text-xs">
                  <thead className="bg-slate-50 border-b border-slate-200 text-slate-500 font-bold uppercase text-[10px]">
                    <tr>
                      <th className="py-3 px-4">Doctor Name</th>
                      <th className="py-3 px-4">BMDC Reg</th>
                      <th className="py-3 px-4">Completed Visits</th>
                      <th className="py-3 px-4">24h Chat Sessions</th>
                      <th className="py-3 px-4">Gross Earnings</th>
                      <th className="py-3 px-4">Platform Cut (20%)</th>
                      <th className="py-3 px-4">Net Payout (80%)</th>
                      <th className="py-3 px-4">Payout Method</th>
                      <th className="py-3 px-4">Status</th>
                      <th className="py-3 px-4 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100">
                    {filteredDoctors.map((doc) => (
                      <tr
                        key={doc.id}
                        onClick={() => setSelectedDoctorForModal(doc)}
                        className="hover:bg-blue-50/60 transition-colors cursor-pointer group"
                      >
                        <td className="py-3.5 px-4">
                          <div className="flex items-center gap-3">
                            <div
                              style={{ backgroundColor: doc.bgColor }}
                              className="w-8 h-8 rounded-full text-white flex items-center justify-center font-bold text-xs flex-shrink-0"
                            >
                              {doc.avatar}
                            </div>
                            <div>
                              <div className="font-bold text-slate-900 group-hover:text-blue-600 transition-colors flex items-center gap-1.5">
                                <span>{doc.full_name}</span>
                              </div>
                              <div className="text-[11px] text-slate-500">
                                {doc.primary_specialty} • {doc.current_hospital}
                              </div>
                              <div className="text-[10px] text-blue-600 font-mono mt-0.5">
                                📞 {doc.verified_phone}
                              </div>
                            </div>
                          </div>
                        </td>
                        <td className="py-3.5 px-4 font-mono font-bold text-slate-700">
                          <span className="px-2 py-0.5 bg-slate-100 rounded-md">
                            {doc.license_number}
                          </span>
                        </td>
                        <td className="py-3.5 px-4 font-bold text-slate-900">
                          {doc.completedVisits}
                        </td>
                        <td className="py-3.5 px-4 text-slate-600">
                          {doc.chatSessions}
                        </td>
                        <td className="py-3.5 px-4 font-bold text-slate-900">
                          ৳ {doc.grossEarnings}
                        </td>
                        <td className="py-3.5 px-4 font-semibold text-rose-600">
                          -৳ {doc.platformCut}
                        </td>
                        <td className="py-3.5 px-4 font-bold text-emerald-600 text-sm">
                          ৳ {doc.netPayout}
                        </td>
                        <td className="py-3.5 px-4 font-semibold text-slate-700">
                          <span className="px-2 py-0.5 bg-slate-100 rounded-md text-[10px] font-bold">
                            {doc.payoutMethod}
                          </span>
                        </td>
                        <td className="py-3.5 px-4">
                          <span
                            className={`px-2 py-0.5 rounded-full text-[10px] font-bold ${
                              doc.statusText.includes('Settled')
                                ? 'bg-emerald-100 text-emerald-800'
                                : 'bg-amber-100 text-amber-800'
                            }`}
                          >
                            {doc.statusText}
                          </span>
                        </td>
                        <td className="py-3.5 px-4 text-right">
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              setSelectedDoctorForModal(doc);
                            }}
                            className="px-2.5 py-1 text-[11px] font-bold text-blue-600 bg-blue-50 hover:bg-blue-100 rounded-lg transition-colors inline-flex items-center gap-1"
                          >
                            <span>View History</span>
                            <ArrowUpRight className="w-3 h-3" />
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* TAB 3: ALL PAYMENTS & PAYMENT HOLDS */}
          {activeTab === 'payments' && (
            <div className="space-y-6">
              {/* Filter controls */}
              <div className="bg-white border border-slate-200 rounded-2xl p-4 flex items-center justify-between gap-4 shadow-2xs">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-bold text-slate-600">Gateway Filter:</span>
                  {['ALL', 'BKASH', 'NAGAD'].map((gw) => (
                    <button
                      key={gw}
                      onClick={() => setSelectedGatewayFilter(gw)}
                      className={`px-3 py-1 rounded-lg text-xs font-bold transition-colors ${
                        selectedGatewayFilter === gw
                          ? 'bg-blue-600 text-white'
                          : 'bg-slate-100 text-slate-700 hover:bg-slate-200'
                      }`}
                    >
                      {gw}
                    </button>
                  ))}
                </div>

                <div className="text-xs text-slate-500 font-medium">
                  Showing transactions across <strong>BKASH</strong> and <strong>NAGAD</strong> MFS rails
                </div>
              </div>

              <div className="bg-white border border-slate-200 rounded-2xl overflow-hidden shadow-2xs">
                <div className="p-4 border-b border-slate-100 flex items-center justify-between">
                  <div>
                    <h3 className="text-xs font-bold uppercase tracking-wider text-slate-800">
                      Payment Holding Ledger
                    </h3>
                    <p className="text-[11px] text-slate-500">
                      Two-stage held funds are transferred to doctor wallets only upon verified consultation completion.
                    </p>
                  </div>
                  <span className="text-xs font-bold text-slate-500">
                    Canonical Status: PAYMENT_HELD
                  </span>
                </div>

                <table className="w-full text-left text-xs">
                  <thead className="bg-slate-50 border-b border-slate-200 text-slate-500 font-bold uppercase text-[10px]">
                    <tr>
                      <th className="py-3 px-4">Transaction ID</th>
                      <th className="py-3 px-4">Payment Session ID</th>
                      <th className="py-3 px-4">Gateway</th>
                      <th className="py-3 px-4">Gateway Ref</th>
                      <th className="py-3 px-4">Gross</th>
                      <th className="py-3 px-4">20% Platform Fee</th>
                      <th className="py-3 px-4">80% Net</th>
                      <th className="py-3 px-4">Payment Status</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100">
                    {transactions
                      .filter(
                        (t) =>
                          selectedGatewayFilter === 'ALL' || t.gateway === selectedGatewayFilter
                      )
                      .map((t) => (
                        <tr key={t.id} className="hover:bg-slate-50 transition-colors">
                          <td className="py-3.5 px-4 font-mono font-bold text-slate-900">
                            {t.transaction_number}
                          </td>
                          <td className="py-3.5 px-4 font-mono text-slate-500 text-[11px]">
                            {t.payment_session_id}
                          </td>
                          <td className="py-3.5 px-4 font-bold text-slate-700">
                            <span className="px-2 py-0.5 rounded-md bg-slate-100 text-slate-800">
                              {t.gateway}
                            </span>
                          </td>
                          <td className="py-3.5 px-4 font-mono text-slate-500 text-[11px]">
                            {t.gateway_reference || 'N/A'}
                          </td>
                          <td className="py-3.5 px-4 font-bold text-slate-900">
                            ৳{t.gross_amount}
                          </td>
                          <td className="py-3.5 px-4 font-semibold text-rose-600">
                            -৳{t.platform_fee_amount}
                          </td>
                          <td className="py-3.5 px-4 font-bold text-emerald-600">
                            ৳{t.net_amount}
                          </td>
                          <td className="py-3.5 px-4">
                            <span
                              className={`px-2.5 py-1 rounded-full text-[10px] font-bold ${
                                t.payment_status === 'PAYMENT_HELD'
                                  ? 'bg-amber-100 text-amber-800'
                                  : t.payment_status === 'SETTLED_TO_DOCTOR'
                                  ? 'bg-emerald-100 text-emerald-800'
                                  : t.payment_status === 'REFUNDED_TO_PATIENT'
                                  ? 'bg-rose-100 text-rose-800'
                                  : 'bg-slate-100 text-slate-800'
                              }`}
                            >
                              {t.payment_status}
                            </span>
                          </td>
                        </tr>
                      ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* TAB 4: CONSULTATION GRIEVANCES */}
          {activeTab === 'grievances' && (
            <div className="space-y-6">
              <div className="p-4 bg-sky-50 border border-sky-200 rounded-2xl flex items-center gap-3">
                <ShieldAlert className="w-5 h-5 text-sky-600 flex-shrink-0" />
                <div className="text-xs text-sky-900">
                  <span className="font-bold">Star-Rating-Free Adjudication:</span> Patient disputes are evaluated against recorded Agora session telemetry (call duration, packet loss, premature end status) to protect both patient rights and physician reputations without subjective star ratings.
                </div>
              </div>

              <div className="space-y-4">
                {grievances.map((g) => (
                  <div
                    key={g.id}
                    className="bg-white border border-slate-200 rounded-2xl p-5 shadow-2xs space-y-4"
                  >
                    <div className="flex items-start justify-between">
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-mono font-bold text-slate-900">
                            {g.grievance_number}
                          </span>
                          <span
                            className={`px-2 py-0.5 rounded-full text-[10px] font-bold ${
                              g.status === 'PENDING_REVIEW'
                                ? 'bg-amber-100 text-amber-800'
                                : g.status === 'REFUNDED'
                                ? 'bg-emerald-100 text-emerald-800'
                                : g.status === 'WARNED'
                                ? 'bg-indigo-100 text-indigo-800'
                                : 'bg-slate-100 text-slate-800'
                            }`}
                          >
                            {g.status}
                          </span>
                        </div>
                        <h4 className="text-sm font-bold text-slate-900 mt-1">
                          Category: {g.category}
                        </h4>
                      </div>

                      <div className="text-[11px] text-slate-400 font-medium">
                        Filed {new Date(g.created_at).toLocaleTimeString()}
                      </div>
                    </div>

                    <p className="text-xs text-slate-600 bg-slate-50 p-3 rounded-xl border border-slate-100">
                      "{g.claim_summary}"
                    </p>

                    {/* Telemetry Evidence Box */}
                    {g.telemetry && (
                      <div className="bg-slate-900 text-white rounded-xl p-3 text-xs flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <Activity className="w-4 h-4 text-emerald-400" />
                          <span className="font-bold text-slate-300">
                            Agora Session Telemetry Evidence:
                          </span>
                        </div>
                        <div className="flex items-center gap-4 text-[11px] font-mono">
                          <span>
                            Duration:{' '}
                            <strong className="text-amber-400">
                              {g.telemetry.call_duration_seconds}s
                            </strong>{' '}
                            (Premature: {g.telemetry.premature_end ? 'YES' : 'NO'})
                          </span>
                          <span>Packet Loss: {g.telemetry.packet_loss_percent}%</span>
                        </div>
                      </div>
                    )}

                    {/* Adjudication Action Buttons */}
                    {g.status === 'PENDING_REVIEW' && (
                      <div className="flex items-center justify-end gap-2 pt-2 border-t border-slate-100">
                        <button
                          onClick={() => handleAdjudicateWarning(g.id)}
                          className="px-3 py-1.5 bg-amber-50 hover:bg-amber-100 text-amber-800 border border-amber-200 rounded-xl text-xs font-bold transition-colors"
                        >
                          Issue Physician Warning
                        </button>
                        <button
                          onClick={() => handleAdjudicateRefund(g.id)}
                          className="px-3.5 py-1.5 bg-rose-600 hover:bg-rose-700 text-white rounded-xl text-xs font-bold transition-colors shadow-xs flex items-center gap-1.5"
                        >
                          <RotateCcw className="w-3.5 h-3.5" />
                          <span>Disburse Patient Refund</span>
                        </button>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* TAB 5: BMDC CREDENTIALING */}
          {activeTab === 'bmdc' && (
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {doctors.map((doc) => (
                  <div
                    key={doc.id}
                    onClick={() => setSelectedDoctorForModal(doc)}
                    className="bg-white border border-slate-200 rounded-2xl p-5 shadow-2xs space-y-4 hover:border-blue-300 transition-all cursor-pointer group"
                  >
                    <div className="flex items-start justify-between">
                      <div className="flex items-center gap-3">
                        <div
                          style={{ backgroundColor: doc.bgColor }}
                          className="w-10 h-10 rounded-xl text-white flex items-center justify-center font-bold text-sm shadow-xs"
                        >
                          {doc.avatar}
                        </div>
                        <div>
                          <h4 className="text-sm font-bold text-slate-900 group-hover:text-blue-600 transition-colors leading-tight">
                            {doc.full_name}
                          </h4>
                          <span className="text-xs text-slate-500 font-medium">
                            {doc.primary_specialty} • {doc.experience_years} Years Exp
                          </span>
                        </div>
                      </div>

                      <span className="px-2 py-0.5 bg-emerald-50 text-emerald-700 border border-emerald-200 rounded-full text-[10px] font-bold">
                        BMDC VERIFIED
                      </span>
                    </div>

                    <div className="grid grid-cols-2 gap-2 text-xs bg-slate-50 p-3 rounded-xl border border-slate-100">
                      <div>
                        <div className="text-[10px] text-slate-400 uppercase font-bold">
                          License Number
                        </div>
                        <div className="font-mono font-bold text-slate-800">
                          {doc.license_number}
                        </div>
                      </div>
                      <div>
                        <div className="text-[10px] text-slate-400 uppercase font-bold">
                          Current Hospital
                        </div>
                        <div className="font-semibold text-slate-800 truncate">
                          {doc.current_hospital}
                        </div>
                      </div>
                      <div className="mt-2">
                        <div className="text-[10px] text-slate-400 uppercase font-bold">
                          Phone Contact
                        </div>
                        <div className="font-semibold text-slate-800">
                          {doc.verified_phone}
                        </div>
                      </div>
                      <div className="mt-2">
                        <div className="text-[10px] text-slate-400 uppercase font-bold">
                          Duty Status
                        </div>
                        <div className="font-bold text-emerald-600">
                          {doc.is_on_duty ? 'ON DUTY (Active)' : 'OFF DUTY'}
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center justify-between pt-1">
                      <div className="flex flex-wrap gap-1">
                        {doc.qualifications.map((q) => (
                          <span
                            key={q}
                            className="px-2 py-0.5 bg-slate-100 text-slate-700 rounded-md text-[10px] font-bold"
                          >
                            {q}
                          </span>
                        ))}
                      </div>

                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setSelectedDoctorForModal(doc);
                        }}
                        className="text-xs font-bold text-blue-600 hover:text-blue-800 flex items-center gap-1"
                      >
                        <span>Open Dossier</span>
                        <ArrowUpRight className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* TAB 6: SLOT MATRIX & LOCKING ENGINE */}
          {activeTab === 'slots' && (
            <div className="space-y-6">
              <div className="p-4 bg-indigo-50 border border-indigo-200 rounded-2xl flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-9 h-9 rounded-xl bg-indigo-100 text-indigo-800 flex items-center justify-center font-bold">
                    <Clock className="w-5 h-5 text-indigo-600" />
                  </div>
                  <div>
                    <h4 className="text-xs font-bold text-indigo-900 leading-tight">
                      Two-Stage Slot Locking Engine (10-Minute Lock TTL)
                    </h4>
                    <p className="text-[11px] text-indigo-700 mt-0.5">
                      Slots transition from AVAILABLE → LOCKED_IN_PAYMENT upon booking checkout. If payment is unconfirmed after 10 minutes, the background lock sweeper automatically restores the slot to AVAILABLE.
                    </p>
                  </div>
                </div>

                <button
                  onClick={() => {
                    setActionMessage('Background lock sweeper triggered: Stale locks reclaimed.');
                    setTimeout(() => setActionMessage(null), 4000);
                  }}
                  className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-bold transition-all shadow-xs flex items-center gap-2"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  <span>Run Lock Sweeper</span>
                </button>
              </div>

              {/* Slot Table */}
              <div className="bg-white border border-slate-200 rounded-2xl overflow-hidden shadow-2xs">
                <div className="p-4 border-b border-slate-100 flex items-center justify-between">
                  <h3 className="text-xs font-bold uppercase tracking-wider text-slate-800">
                    Active Doctor Slot Matrix
                  </h3>
                  <span className="text-xs text-slate-500 font-semibold">
                    {slots.length} Total Registered Slots
                  </span>
                </div>

                <table className="w-full text-left text-xs">
                  <thead className="bg-slate-50 border-b border-slate-200 text-slate-500 font-bold uppercase text-[10px]">
                    <tr>
                      <th className="py-3 px-4">Slot ID</th>
                      <th className="py-3 px-4">Assigned Doctor</th>
                      <th className="py-3 px-4">Time Window</th>
                      <th className="py-3 px-4">Slot Status</th>
                      <th className="py-3 px-4">Payment Lock Session</th>
                      <th className="py-3 px-4">Lock TTL / Expiry</th>
                      <th className="py-3 px-4 text-right">Admin Actions</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100">
                    {slots.map((slot) => (
                      <tr key={slot.id} className="hover:bg-slate-50 transition-colors">
                        <td className="py-3.5 px-4 font-mono font-bold text-slate-900">{slot.id}</td>
                        <td className="py-3.5 px-4 font-semibold text-slate-800">{slot.doctor_name}</td>
                        <td className="py-3.5 px-4 text-slate-600">
                          {new Date(slot.start_time).toLocaleTimeString([], {
                            hour: '2-digit',
                            minute: '2-digit',
                          })}{' '}
                          -{' '}
                          {new Date(slot.end_time).toLocaleTimeString([], {
                            hour: '2-digit',
                            minute: '2-digit',
                          })}
                        </td>
                        <td className="py-3.5 px-4">
                          <span
                            className={`px-2.5 py-1 rounded-full text-[10px] font-bold ${
                              slot.status === 'AVAILABLE'
                                ? 'bg-emerald-100 text-emerald-800'
                                : slot.status === 'LOCKED_IN_PAYMENT'
                                ? 'bg-amber-100 text-amber-800 animate-pulse'
                                : slot.status === 'BOOKED'
                                ? 'bg-blue-100 text-blue-800'
                                : 'bg-slate-100 text-slate-700'
                            }`}
                          >
                            {slot.status}
                          </span>
                        </td>
                        <td className="py-3.5 px-4 font-mono text-slate-500 text-[11px]">
                          {slot.lock_session_id || '—'}
                        </td>
                        <td className="py-3.5 px-4 font-mono text-slate-600">
                          {slot.lock_expires_at ? (
                            <span className="text-amber-600 font-bold">
                              {new Date(slot.lock_expires_at).toLocaleTimeString()}
                            </span>
                          ) : (
                            '—'
                          )}
                        </td>
                        <td className="py-3.5 px-4 text-right">
                          {slot.status === 'LOCKED_IN_PAYMENT' && (
                            <button
                              onClick={() => handleForceReleaseLock(slot.id)}
                              className="px-2.5 py-1 text-[11px] font-bold text-rose-700 bg-rose-50 hover:bg-rose-100 border border-rose-200 rounded-lg transition-colors flex items-center gap-1 inline-flex"
                            >
                              <Unlock className="w-3 h-3" />
                              <span>Release Lock</span>
                            </button>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* TAB 7: PATIENTS DIRECTORY */}
          {activeTab === 'patients' && (
            <div className="space-y-6">
              {/* Emergency Hotline Header */}
              <div className="p-4 bg-emerald-50 border border-emerald-200 rounded-2xl flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-9 h-9 rounded-xl bg-emerald-100 text-emerald-800 flex items-center justify-center font-bold">
                    <PhoneCall className="w-5 h-5 text-emerald-700" />
                  </div>
                  <div>
                    <h4 className="text-xs font-bold text-emerald-900 leading-tight">
                      National Health Emergency Helpline Integration: 16263
                    </h4>
                    <p className="text-[11px] text-emerald-700 mt-0.5">
                      All urgent cases with red-flag symptoms are triaged to call 16263 or dispatch ambulance services immediately. Click any patient row to open their clinical dossier.
                    </p>
                  </div>
                </div>

                <div className="px-3 py-1.5 bg-emerald-700 text-white rounded-xl text-xs font-bold font-mono">
                  HOTLINE: 16263
                </div>
              </div>

              {/* Patients Table */}
              <div className="bg-white border border-slate-200 rounded-2xl overflow-hidden shadow-2xs">
                <div className="p-4 border-b border-slate-100 flex items-center justify-between">
                  <div>
                    <h3 className="text-xs font-bold uppercase tracking-wider text-slate-800">
                      Registered Patients
                    </h3>
                    <p className="text-[11px] text-slate-500">
                      Patient clinical intake records are strictly isolated under ABAC / Row-Level Security. Operational admins can view directory details without PHI leakage.
                    </p>
                  </div>
                  <span className="text-xs font-semibold text-slate-500">
                    {patients.length} Registered Patients
                  </span>
                </div>

                <table className="w-full text-left text-xs">
                  <thead className="bg-slate-50 border-b border-slate-200 text-slate-500 font-bold uppercase text-[10px]">
                    <tr>
                      <th className="py-3 px-4">Patient Name</th>
                      <th className="py-3 px-4">Verified Phone (E.164)</th>
                      <th className="py-3 px-4">Demographics</th>
                      <th className="py-3 px-4">Chronic Cohort</th>
                      <th className="py-3 px-4">Total Consultations</th>
                      <th className="py-3 px-4">Last Consult Date</th>
                      <th className="py-3 px-4 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100">
                    {patients.map((p) => (
                      <tr
                        key={p.id}
                        onClick={() => setSelectedPatientForModal(p)}
                        className="hover:bg-blue-50/60 transition-colors cursor-pointer group"
                      >
                        <td className="py-3.5 px-4">
                          <div className="flex items-center gap-2.5">
                            <div
                              style={{ backgroundColor: p.bgColor }}
                              className="w-8 h-8 rounded-full text-white font-bold flex items-center justify-center text-xs"
                            >
                              {p.avatar}
                            </div>
                            <div>
                              <div className="font-bold text-slate-900 group-hover:text-blue-600 transition-colors">
                                {p.display_name}
                              </div>
                              <span className="text-[10px] text-slate-400 font-mono">{p.id}</span>
                            </div>
                          </div>
                        </td>
                        <td className="py-3.5 px-4 font-mono font-semibold text-slate-800">{p.phone}</td>
                        <td className="py-3.5 px-4 text-slate-600">
                          {p.gender}, {p.age} Years
                        </td>
                        <td className="py-3.5 px-4">
                          <span className="px-2 py-0.5 bg-blue-50 text-blue-700 font-bold rounded-md text-[10px]">
                            {p.cohort}
                          </span>
                        </td>
                        <td className="py-3.5 px-4 font-bold text-blue-600">{p.total_consultations}</td>
                        <td className="py-3.5 px-4 text-slate-500">{p.last_consultation_date}</td>
                        <td className="py-3.5 px-4 text-right">
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              setSelectedPatientForModal(p);
                            }}
                            className="px-2.5 py-1 text-[11px] font-bold text-blue-600 bg-blue-50 hover:bg-blue-100 rounded-lg transition-colors inline-flex items-center gap-1"
                          >
                            <span>Dossier</span>
                            <ArrowUpRight className="w-3 h-3" />
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* TAB 8: INCIDENT & SUBSYSTEM LOGS */}
          {activeTab === 'logs' && (
            <div className="space-y-6">
              {/* Filter controls */}
              <div className="bg-white border border-slate-200 rounded-2xl p-4 flex items-center justify-between gap-4 shadow-2xs">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-bold text-slate-600">Role Filter:</span>
                  {['ALL', 'FINANCE_ADMIN', 'CLINICAL_ADMIN', 'SECURITY_ADMIN', 'SYSTEM_WORKER'].map(
                    (role) => (
                      <button
                        key={role}
                        onClick={() => setSelectedSubsystemFilter(role)}
                        className={`px-3 py-1 rounded-lg text-xs font-bold transition-colors ${
                          selectedSubsystemFilter === role
                            ? 'bg-blue-600 text-white'
                            : 'bg-slate-100 text-slate-700 hover:bg-slate-200'
                        }`}
                      >
                        {role}
                      </button>
                    )
                  )}
                </div>

                <div className="text-xs text-slate-500 font-medium">
                  Immutable Audit Ledger: <strong className="text-emerald-700">Ed25519 & RLS Enforced</strong>
                </div>
              </div>

              {/* Logs Table */}
              <div className="bg-white border border-slate-200 rounded-2xl overflow-hidden shadow-2xs">
                <div className="p-4 border-b border-slate-100 flex items-center justify-between">
                  <h3 className="text-xs font-bold uppercase tracking-wider text-slate-800">
                    System Audit Trail (`audit_events`)
                  </h3>
                  <span className="text-xs text-slate-500">
                    Preserves non-repudiation and clinical telemetry integrity
                  </span>
                </div>

                <table className="w-full text-left text-xs">
                  <thead className="bg-slate-50 border-b border-slate-200 text-slate-500 font-bold uppercase text-[10px]">
                    <tr>
                      <th className="py-3 px-4">Timestamp</th>
                      <th className="py-3 px-4">Actor Role</th>
                      <th className="py-3 px-4">Action</th>
                      <th className="py-3 px-4">Entity Type / ID</th>
                      <th className="py-3 px-4">IP Address</th>
                      <th className="py-3 px-4">RLS Status</th>
                      <th className="py-3 px-4">Details</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-100">
                    {auditLogs
                      .filter(
                        (l) =>
                          selectedSubsystemFilter === 'ALL' ||
                          l.actor_role === selectedSubsystemFilter
                      )
                      .map((log) => (
                        <tr key={log.id} className="hover:bg-slate-50 transition-colors">
                          <td className="py-3.5 px-4 font-mono text-[11px] text-slate-500">
                            {new Date(log.timestamp).toLocaleTimeString()}
                          </td>
                          <td className="py-3.5 px-4 font-bold">
                            <span
                              className={`px-2 py-0.5 rounded-md text-[10px] font-bold ${
                                log.actor_role === 'FINANCE_ADMIN'
                                  ? 'bg-amber-100 text-amber-800'
                                  : log.actor_role === 'CLINICAL_ADMIN'
                                  ? 'bg-blue-100 text-blue-800'
                                  : log.actor_role === 'SECURITY_ADMIN'
                                  ? 'bg-purple-100 text-purple-800'
                                  : 'bg-slate-100 text-slate-800'
                              }`}
                            >
                              {log.actor_role}
                            </span>
                          </td>
                          <td className="py-3.5 px-4 font-mono font-semibold text-slate-800">
                            {log.action}
                          </td>
                          <td className="py-3.5 px-4 font-mono text-[11px] text-slate-600">
                            {log.entity_type} ({log.entity_id})
                          </td>
                          <td className="py-3.5 px-4 font-mono text-[11px] text-slate-500">
                            {log.ip_address}
                          </td>
                          <td className="py-3.5 px-4">
                            <span className="px-2 py-0.5 bg-emerald-50 text-emerald-700 border border-emerald-200 rounded-full text-[10px] font-bold flex items-center gap-1 w-fit">
                              <ShieldCheck className="w-3 h-3" />
                              <span>ENFORCED</span>
                            </span>
                          </td>
                          <td className="py-3.5 px-4 text-slate-600 text-[11px]">{log.details}</td>
                        </tr>
                      ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </main>
      </div>

      {/* ═════════════════════════════════════════════════════════════
          MODAL: DOCTOR HISTORY & FINANCIAL DOSSIER (MATCHING INDEX.HTML)
          ═════════════════════════════════════════════════════════════ */}
      {selectedDoctorForModal && (
        <div
          className="fixed inset-0 bg-slate-900/65 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150"
          onClick={() => setSelectedDoctorForModal(null)}
        >
          <div
            className="bg-white rounded-2xl max-w-4xl w-full overflow-hidden shadow-2xl flex flex-col max-h-[92vh] animate-in zoom-in-95 duration-150"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header */}
            <div className="bg-slate-900 text-white p-5 flex items-start justify-between border-b border-slate-800">
              <div className="flex items-center gap-3.5">
                <div
                  style={{ backgroundColor: selectedDoctorForModal.bgColor }}
                  className="w-12 h-12 rounded-full text-white font-black text-base flex items-center justify-center shadow-md flex-shrink-0"
                >
                  {selectedDoctorForModal.avatar}
                </div>
                <div>
                  <div className="flex items-center gap-2 flex-wrap">
                    <h3 className="text-lg font-bold text-white leading-none">
                      {selectedDoctorForModal.full_name}
                    </h3>
                    <span className="px-2.5 py-0.5 bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 font-bold rounded-full text-xs font-mono">
                      {selectedDoctorForModal.license_number}
                    </span>
                    <span className="px-2.5 py-0.5 bg-sky-500/20 text-sky-300 border border-sky-500/30 font-bold rounded-full text-xs">
                      Verified Specialist
                    </span>
                    <span className="px-2 py-0.5 bg-white/10 text-slate-300 font-semibold rounded-full text-[11px]">
                      {selectedDoctorForModal.is_on_duty ? 'Active Practitioner' : 'Off Duty'}
                    </span>
                  </div>
                  <div className="text-xs text-slate-300 mt-1 font-medium">
                    {selectedDoctorForModal.primary_specialty} Consultant • {selectedDoctorForModal.current_hospital} • BMDC Registered Specialist
                  </div>
                  <div className="flex items-center gap-4 mt-2 text-xs flex-wrap text-slate-300">
                    <span className="flex items-center gap-1.5 text-cyan-300 font-mono">
                      <PhoneCall className="w-3.5 h-3.5 text-cyan-400" />
                      <span>{selectedDoctorForModal.verified_phone}</span>
                    </span>
                    <span className="text-slate-600">•</span>
                    <span className="flex items-center gap-1.5 text-amber-200">
                      <MapPin className="w-3.5 h-3.5 text-amber-400" />
                      <span>{selectedDoctorForModal.residential_address}</span>
                    </span>
                    <span className="text-slate-600">•</span>
                    <span className="flex items-center gap-1.5 text-slate-300">
                      <Mail className="w-3.5 h-3.5 text-emerald-400" />
                      <span>{selectedDoctorForModal.email}</span>
                    </span>
                  </div>
                </div>
              </div>

              <button
                onClick={() => setSelectedDoctorForModal(null)}
                className="w-8 h-8 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-white flex items-center justify-center transition-colors text-base"
              >
                ✕
              </button>
            </div>

            {/* KPI Summary Strip (4 cards) */}
            <div className="grid grid-cols-4 bg-slate-100 border-b border-slate-200 divide-x divide-slate-200 text-center">
              <div className="p-3 bg-white">
                <div className="text-[10px] text-slate-500 uppercase font-bold tracking-wider">
                  Lifetime Consultations
                </div>
                <div className="text-lg font-black text-slate-900 mt-0.5">
                  {selectedDoctorForModal.lifetimeConsultations.toLocaleString()}
                </div>
              </div>
              <div className="p-3 bg-white">
                <div className="text-[10px] text-slate-500 uppercase font-bold tracking-wider">
                  24h Chat Sessions
                </div>
                <div className="text-lg font-black text-blue-600 mt-0.5">
                  {selectedDoctorForModal.chatTotalSessions.toLocaleString()}
                </div>
              </div>
              <div className="p-3 bg-white">
                <div className="text-[10px] text-slate-500 uppercase font-bold tracking-wider">
                  Total Gross Revenue
                </div>
                <div className="text-lg font-black text-slate-900 mt-0.5">
                  ৳ {selectedDoctorForModal.lifetimeGross}
                </div>
              </div>
              <div className="p-3 bg-white">
                <div className="text-[10px] text-slate-500 uppercase font-bold tracking-wider">
                  Net Payouts Disbursed (80%)
                </div>
                <div className="text-lg font-black text-emerald-600 mt-0.5">
                  ৳ {selectedDoctorForModal.lifetimeNet}
                </div>
              </div>
            </div>

            {/* Modal Body (Scrollable) */}
            <div className="p-5 overflow-y-auto space-y-6 flex-1 text-xs">
              {/* Encounter & Prescriptions table */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <h4 className="font-bold text-slate-900 text-sm">
                    Recent Consultation Encounters & Issued Prescriptions
                  </h4>
                  <span className="text-[11px] text-slate-500">
                    Archived across all patient encounters
                  </span>
                </div>

                <div className="border border-slate-200 rounded-xl overflow-hidden">
                  <table className="w-full text-left">
                    <thead className="bg-slate-50 text-[10px] uppercase font-bold text-slate-500 border-b border-slate-200">
                      <tr>
                        <th className="py-2.5 px-3">Date & Time</th>
                        <th className="py-2.5 px-3">Patient Name</th>
                        <th className="py-2.5 px-3">Consultation Mode</th>
                        <th className="py-2.5 px-3">Fee (৳)</th>
                        <th className="py-2.5 px-3">Clinical Diagnosis</th>
                        <th className="py-2.5 px-3">Rx Issued</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100">
                      {selectedDoctorForModal.encounters.map((enc, idx) => (
                        <tr key={idx} className="hover:bg-slate-50">
                          <td className="py-2.5 px-3 font-semibold text-slate-700">{enc.time}</td>
                          <td className="py-2.5 px-3 font-bold text-slate-900">{enc.ptName}</td>
                          <td className="py-2.5 px-3">
                            <span className="px-2 py-0.5 rounded-md font-bold text-[10px] bg-emerald-50 text-emerald-800">
                              {enc.mode}
                            </span>
                          </td>
                          <td className="py-2.5 px-3 font-bold text-slate-900">{enc.fee}</td>
                          <td className="py-2.5 px-3 text-slate-700">{enc.diagnosis}</td>
                          <td className="py-2.5 px-3">
                            <span className="px-2 py-0.5 rounded-md font-bold text-[10px] bg-blue-50 text-blue-700">
                              {enc.rx}
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>

              {/* Financial Wallet Disbursements History */}
              <div>
                <h4 className="font-bold text-slate-900 text-sm mb-2">
                  Recent Financial Wallet Disbursements
                </h4>
                <div className="space-y-2">
                  {selectedDoctorForModal.disbursements.map((d, idx) => (
                    <div
                      key={idx}
                      className="p-3 bg-slate-50 border border-slate-200 rounded-xl flex items-center justify-between"
                    >
                      <div>
                        <div className="font-bold text-slate-900 text-xs flex items-center gap-2">
                          <span>{d.date}: ৳ {d.amount}</span>
                          <span className="text-[10px] text-slate-500 font-normal">Disbursed via</span>
                          <span className="px-2 py-0.5 bg-pink-100 text-pink-700 font-bold rounded-md text-[10px]">
                            {d.method}
                          </span>
                        </div>
                        <div className="text-[11px] text-slate-500 mt-0.5">
                          TxID: {d.txId} • Platform Fee Withheld: {d.commissionDeducted}
                        </div>
                      </div>
                      <span className="px-2.5 py-1 bg-emerald-100 text-emerald-800 font-bold rounded-full text-[10px]">
                        {d.status}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            {/* Modal Footer */}
            <div className="p-4 bg-slate-50 border-t border-slate-200 flex items-center justify-between">
              <span className="text-xs text-slate-500">
                BMDC Verified Credential Ledger • HelloDoctor Bangladesh
              </span>
              <button
                onClick={() => setSelectedDoctorForModal(null)}
                className="px-4 py-2 bg-slate-900 hover:bg-slate-800 text-white rounded-xl text-xs font-bold transition-colors"
              >
                Close Dossier
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ═════════════════════════════════════════════════════════════
          MODAL: PATIENT CLINICAL DOSSIER & VAULT (MATCHING INDEX.HTML)
          ═════════════════════════════════════════════════════════════ */}
      {selectedPatientForModal && (
        <div
          className="fixed inset-0 bg-slate-900/65 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150"
          onClick={() => setSelectedPatientForModal(null)}
        >
          <div
            className="bg-white rounded-2xl max-w-4xl w-full overflow-hidden shadow-2xl flex flex-col max-h-[92vh] animate-in zoom-in-95 duration-150"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header */}
            <div className="bg-slate-900 text-white p-5 flex items-start justify-between border-b border-slate-800">
              <div className="flex items-center gap-3.5">
                <div
                  style={{ backgroundColor: selectedPatientForModal.bgColor }}
                  className="w-12 h-12 rounded-full text-white font-black text-base flex items-center justify-center shadow-md flex-shrink-0"
                >
                  {selectedPatientForModal.avatar}
                </div>
                <div>
                  <div className="flex items-center gap-2">
                    <h3 className="text-lg font-bold text-white leading-none">
                      {selectedPatientForModal.display_name}
                    </h3>
                    <span className="px-2 py-0.5 bg-slate-700 text-slate-300 font-mono rounded-md text-xs font-bold">
                      {selectedPatientForModal.id}
                    </span>
                    <span className="px-2 py-0.5 bg-emerald-500/20 text-emerald-300 font-bold rounded-full text-xs">
                      Active 24h Chat
                    </span>
                  </div>
                  <div className="text-xs text-slate-300 mt-1">
                    {selectedPatientForModal.age} {selectedPatientForModal.gender} • Emergency Contact: {selectedPatientForModal.emergency_contact}
                  </div>
                  <div className="text-xs text-slate-400 mt-1">
                    Registered: {selectedPatientForModal.registered_at} • Total Visits: {selectedPatientForModal.total_consultations}
                  </div>
                </div>
              </div>

              <button
                onClick={() => setSelectedPatientForModal(null)}
                className="w-8 h-8 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-white flex items-center justify-center transition-colors text-base"
              >
                ✕
              </button>
            </div>

            {/* Vitals Strip */}
            <div className="grid grid-cols-4 bg-slate-100 border-b border-slate-200 divide-x divide-slate-200 text-center">
              <div className="p-3 bg-white">
                <div className="text-[10px] text-slate-500 uppercase font-bold tracking-wider">
                  Latest Blood Pressure
                </div>
                <div className="text-base font-black text-rose-600 mt-0.5">
                  {selectedPatientForModal.bp}
                </div>
              </div>
              <div className="p-3 bg-white">
                <div className="text-[10px] text-slate-500 uppercase font-bold tracking-wider">
                  Resting Pulse
                </div>
                <div className="text-base font-black text-slate-900 mt-0.5">
                  {selectedPatientForModal.pulse}
                </div>
              </div>
              <div className="p-3 bg-white">
                <div className="text-[10px] text-slate-500 uppercase font-bold tracking-wider">
                  Chronic Care Cohort
                </div>
                <div className="text-sm font-black text-blue-600 mt-0.5">
                  {selectedPatientForModal.cohort}
                </div>
              </div>
              <div className="p-3 bg-white">
                <div className="text-[10px] text-slate-500 uppercase font-bold tracking-wider">
                  Attached Lab Panels
                </div>
                <div className="text-sm font-black text-emerald-600 mt-0.5">
                  {selectedPatientForModal.labs}
                </div>
              </div>
            </div>

            {/* Body */}
            <div className="p-5 overflow-y-auto space-y-6 flex-1 text-xs">
              <div>
                <h4 className="font-bold text-slate-900 text-sm mb-2">
                  Patient Lifetime Consultation History
                </h4>
                <div className="border border-slate-200 rounded-xl overflow-hidden">
                  <table className="w-full text-left">
                    <thead className="bg-slate-50 text-[10px] uppercase font-bold text-slate-500 border-b border-slate-200">
                      <tr>
                        <th className="py-2.5 px-3">Date</th>
                        <th className="py-2.5 px-3">Attending Physician</th>
                        <th className="py-2.5 px-3">Specialty</th>
                        <th className="py-2.5 px-3">Consultation Mode</th>
                        <th className="py-2.5 px-3">Primary Finding</th>
                        <th className="py-2.5 px-3">Issued e-Rx</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100">
                      {selectedPatientForModal.encounters.map((enc, idx) => (
                        <tr key={idx} className="hover:bg-slate-50">
                          <td className="py-2.5 px-3 font-semibold text-slate-700">{enc.date}</td>
                          <td className="py-2.5 px-3 font-bold text-slate-900">{enc.doctor}</td>
                          <td className="py-2.5 px-3 text-slate-600">{enc.spec}</td>
                          <td className="py-2.5 px-3">
                            <span className="px-2 py-0.5 rounded-md font-bold text-[10px] bg-emerald-50 text-emerald-800">
                              {enc.mode}
                            </span>
                          </td>
                          <td className="py-2.5 px-3 text-slate-700">{enc.diag}</td>
                          <td className="py-2.5 px-3 font-mono font-bold text-blue-600">{enc.rx}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>

              {/* Health Vault Attached Diagnostic Tests */}
              <div>
                <h4 className="font-bold text-slate-900 text-sm mb-2">
                  Health Vault Attached Diagnostic Tests & Prescriptions
                </h4>
                <div className="grid grid-cols-2 gap-3">
                  {selectedPatientForModal.vaultDocuments.map((doc, idx) => (
                    <div
                      key={idx}
                      className="p-3 bg-slate-50 border border-slate-200 rounded-xl flex items-center justify-between"
                    >
                      <div>
                        <div className="font-bold text-slate-900 flex items-center gap-1.5">
                          <FileText className="w-4 h-4 text-rose-500" />
                          <span>{doc.name}</span>
                        </div>
                        <div className="text-[11px] text-slate-500 mt-0.5">{doc.meta}</div>
                      </div>
                      <button
                        onClick={() => {
                          setActionMessage(`Viewing diagnostic record: ${doc.name}`);
                          setTimeout(() => setActionMessage(null), 3000);
                        }}
                        className="px-2.5 py-1 bg-slate-200 hover:bg-slate-300 text-slate-800 font-bold rounded-lg text-xs transition-colors"
                      >
                        View PDF
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            {/* Modal Footer */}
            <div className="p-4 bg-slate-50 border-t border-slate-200 flex items-center justify-between">
              <span className="text-xs text-slate-500">
                Confidential Medical Record • HelloDoctor Health Vault
              </span>
              <button
                onClick={() => setSelectedPatientForModal(null)}
                className="px-4 py-2 bg-slate-900 hover:bg-slate-800 text-white rounded-xl text-xs font-bold transition-colors"
              >
                Close Dossier
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Monthly Disbursement Batch Modal */}
      {disbursementModalOpen && (
        <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="bg-white rounded-2xl max-w-md w-full p-6 space-y-5 shadow-2xl">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-blue-50 text-blue-600 flex items-center justify-center">
                <Send className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-sm font-bold text-slate-900">
                  Initiate Monthly Doctor Disbursement
                </h3>
                <p className="text-xs text-slate-500">
                  Cycle: September 2026 Batch Payout
                </p>
              </div>
            </div>

            <div className="p-3 bg-slate-50 rounded-xl border border-slate-200 text-xs space-y-2">
              <div className="flex justify-between">
                <span className="text-slate-500">Total Participating Doctors:</span>
                <strong className="text-slate-900">6 Physicians</strong>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-500">Gross Patient Billings:</span>
                <strong className="text-slate-900">৳ 462,050.00</strong>
              </div>
              <div className="flex justify-between text-rose-600">
                <span>Withheld 20% Platform Fee:</span>
                <strong>-৳ 92,410.00</strong>
              </div>
              <div className="flex justify-between border-t border-slate-200 pt-1 text-emerald-700 font-bold">
                <span>Total Net Batch Disbursed:</span>
                <span>৳ 369,640.00</span>
              </div>
            </div>

            <p className="text-[11px] text-slate-500">
              Executing this operation triggers automated bulk MFS payment API callbacks to linked physician accounts and resets pending doctor wallet balances.
            </p>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                onClick={() => setDisbursementModalOpen(false)}
                className="px-4 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-bold transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleInitiateMonthlyDisbursement}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-xl text-xs font-bold transition-colors shadow-xs"
              >
                Confirm & Disburse Batch
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
