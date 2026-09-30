/**
 * Clinical Mock Data & State Dictionary
 * Tailored for HIV mHealth in Sub-Saharan Africa
 */

const AppData = {
  patient: {
    name: "Tendai Moyo",
    anonAlias: "Patient #8429",
    age: 34,
    gender: "Male",
    clinicId: "KEN-KNH-8921",
    clinicName: "Kenyatta National Hospital Comprehensive Care Centre",
    adherenceStreak: 48, // 48 consecutive days
    adherencePercent: 98.4,
    viralLoad: {
      latestValue: "< 20",
      unit: "copies/mL",
      status: "Undetectable",
      isSuppressed: true,
      lastTested: "14 Aug 2024",
      nextDue: "14 Feb 2025"
    },
    cd4Count: {
      latestValue: 680,
      unit: "cells/mm³",
      baseline: 210,
      status: "Robust Immune Recovery"
    },
    currentRegimen: {
      code: "TLD",
      name: "Tenofovir + Lamivudine + Dolutegravir",
      shortName: "TLD Fixed-Dose Combination",
      strength: "300mg / 300mg / 50mg",
      frequency: "Once daily (Night)",
      instructions: "Take 1 tablet every night with a glass of water. Best taken after your evening meal.",
      pillAppearance: "Burgundy/Terracotta oval-shaped coated tablet with 'TLD' imprint",
      nextRefillDue: "In 18 Days (3-Month MMD Supply)"
    }
  },

  // Masked Terminology Dictionary for Anti-Stigma / Discreet Mode
  discreetDictionary: {
    standard: {
      appName: "Aura Health",
      tagline: "Your Private Wellness & Care Companion",
      regimenTitle: "Daily ART Regimen",
      medName: "TLD Fixed-Dose Combination",
      medSubtitle: "Tenofovir / Lamivudine / Dolutegravir (1 tab)",
      viralLoadTitle: "Viral Load Status",
      viralLoadValue: "Undetectable (<20 copies/mL)",
      viralLoadTag: "U=U Suppressed",
      consultTitle: "HIV Specialist & Counselor",
      refillTitle: "ART Medication Refill",
      peerTitle: "HIV Peer Navigator",
      safetyAlert: "Missing your ART pill can risk viral rebound. Take immediately if within window."
    },
    discreet: {
      appName: "Aura Daily",
      tagline: "Daily Hydration & Vitality Routine",
      regimenTitle: "Daily Vitality Supplement",
      medName: "Essential Daily Complex",
      medSubtitle: "Multi-nutrient & Botanical Mineral (1 tab)",
      viralLoadTitle: "Metabolic Energy Score",
      viralLoadValue: "Optimal Vitality (<20 index)",
      viralLoadTag: "Peak Health",
      consultTitle: "Personal Nutrition Coach",
      refillTitle: "Wellness Pack Reorder",
      peerTitle: "Community Wellness Buddy",
      safetyAlert: "Take your daily wellness booster to maintain peak daytime energy."
    }
  },

  doctors: [
    {
      id: "doc-1",
      name: "Dr. Amina Osei",
      title: "Senior Infectious Disease Specialist",
      credentials: "MBChB, MMed, FCPath • KMPDC #A4912",
      facility: "Infectious Diseases Institute",
      rating: 4.9,
      reviewsCount: 142,
      fee: "KES 1,200",
      feeUSD: "$9.50",
      avatar: "https://images.unsplash.com/photo-1559839734-2b71ea197ec2?auto=format&fit=crop&q=80&w=200",
      languages: ["English", "Swahili", "Luganda"],
      nextAvailable: "Today at 3:30 PM"
    },
    {
      id: "doc-2",
      name: "Dr. Kwesi Mensah",
      title: "Clinical Pharmacist & ART Toxicologist",
      credentials: "PharmD, MSc Clinical Pharmacy",
      facility: "University Teaching Hospital",
      rating: 4.8,
      reviewsCount: 98,
      fee: "KES 950",
      feeUSD: "$7.50",
      avatar: "https://images.unsplash.com/photo-1622253692010-333f2da6031d?auto=format&fit=crop&q=80&w=200",
      languages: ["English", "Twi", "French"],
      nextAvailable: "Tomorrow at 10:00 AM"
    }
  ],

  peerNavigators: [
    {
      id: "peer-1",
      name: "Sister Grace (Peer Mentor)",
      livedExperience: "8 Years Living Thriving with HIV (U=U)",
      focus: "Stigma navigation, partner disclosure & mental peace",
      avatar: "https://images.unsplash.com/photo-1573496359142-b8d87734a5a2?auto=format&fit=crop&q=80&w=200",
      online: true
    }
  ],

  viralLoadHistory: [
    { date: "Aug 2021", value: 45000, label: "45,000 (Baseline)" },
    { date: "Feb 2022", value: 3800, label: "3,800" },
    { date: "Aug 2022", value: 420, label: "420 (Low)" },
    { date: "Feb 2023", value: 75, label: "75 (Suppressed)" },
    { date: "Aug 2023", value: 18, label: "< 20 (Undetectable)" },
    { date: "Feb 2024", value: 14, label: "< 20 (Undetectable)" },
    { date: "Aug 2024", value: 12, label: "< 20 (Undetectable)" }
  ]
};
