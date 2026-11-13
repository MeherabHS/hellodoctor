import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../core/config/emergency_contact.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../blogs/blogs_page.dart';
import '../blogs/blogs_repository.dart';
import '../home/home_cubit.dart';
import '../home/upcoming_consultation_section.dart';
import '../profile/data_rights_page.dart';
import '../profile/family_profiles_cubit.dart';
import '../qna/qna_ask_sheet.dart';
import '../qna/qna_cubit.dart';
import '../qna/qna_page.dart';
import '../qna/qna_repository.dart';
import '../vault/health_vault_page.dart';
import '../vault/vault_repository.dart';
import 'afya_module_widgets.dart';
import 'patient_modules.dart';

const _pagePadding = EdgeInsets.fromLTRB(20, 6, 20, 24);

// ---------------------------------------------------------------------------
// Panel 2 — HIV Prevention & Sexual Health
// ---------------------------------------------------------------------------

/// Education hub. Learn lists the six featured topics; a topic opens the
/// existing article browser pre-filtered with the topic as its search text
/// (the article feed has no category filter yet). Videos and Audio have no
/// content model, so those tabs say so. Quizzes live inside admin articles.
///
/// Deviation from the panel: the "3 min" reading-time lines are omitted
/// because no such value exists for a topic.
class EducationHubPage extends StatefulWidget {
  const EducationHubPage(
      {super.key, required this.nav, required this.repo, this.initialTab = 0});
  final PatientShellNav nav;
  final BlogsRepository repo;
  final int initialTab;
  @override
  State<EducationHubPage> createState() => _EducationHubPageState();
}

class _EducationHubPageState extends State<EducationHubPage> {
  late int _tab = widget.initialTab;

  void _openArticles([String? query]) =>
      Navigator.of(context).push(MaterialPageRoute(
          builder: (_) => BlogsPage(repo: widget.repo, initialQuery: query)));

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final topics = <(String, String, String, String)>[
      ('topic_hiv_basics', l.eduTopicHivBasics, l.eduTopicHivBasicsDesc, 'HIV'),
      ('topic_prep', l.eduTopicPrep, l.eduTopicPrepDesc, 'PrEP'),
      ('topic_condoms', l.eduTopicCondoms, l.eduTopicCondomsDesc, 'condom'),
      ('topic_srh', l.eduTopicSrh, l.eduTopicSrhDesc, 'reproductive'),
      ('topic_stigma', l.eduTopicStigma, l.eduTopicStigmaDesc, 'stigma'),
      (
        'topic_relationships',
        l.eduTopicRelationships,
        l.eduTopicRelationshipsDesc,
        'relationship'
      ),
    ];
    return AfyaModuleScaffold(
      title: l.moduleHivPrevention,
      nav: widget.nav,
      child: ListView(padding: _pagePadding, children: [
        AfyaSegmentedTabs(labels: [
          l.eduTabLearn,
          l.eduTabVideos,
          l.eduTabQuizzes,
          l.eduTabAudio
        ], selected: _tab, onSelected: (i) => setState(() => _tab = i)),
        const SizedBox(height: 18),
        if (_tab == 0) ...[
          for (final t in topics)
            Padding(
              padding: const EdgeInsets.only(bottom: 16),
              child: AfyaCard(
                color: PatientTokens.cardTint,
                shadow: false,
                padding: EdgeInsets.zero,
                onTap: () => _openArticles(t.$4),
                child: Row(children: [
                  AfyaArt(t.$1, width: 104, height: 104),
                  const SizedBox(width: 14),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 10),
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(t.$2,
                                style: const TextStyle(
                                    fontSize: 14.5,
                                    fontWeight: FontWeight.w700,
                                    color: PatientTokens.navy)),
                            const SizedBox(height: 6),
                            Text(t.$3,
                                style: const TextStyle(
                                    fontSize: 13,
                                    height: 1.4,
                                    color: PatientTokens.ink)),
                          ]),
                    ),
                  ),
                  const Padding(
                    padding: EdgeInsets.symmetric(horizontal: 8),
                    child: Icon(Icons.chevron_right_rounded,
                        color: PatientTokens.navy),
                  ),
                ]),
              ),
            ),
          const SizedBox(height: 4),
          OutlinedButton(
              onPressed: _openArticles, child: Text(l.eduAllArticles)),
        ] else if (_tab == 1)
          AfyaUnavailableNote(
              icon: Icons.play_circle_outline_rounded,
              title: l.moduleUnavailableTitle,
              body: l.eduVideosUnavailable)
        else if (_tab == 2) ...[
          AfyaUnavailableNote(
              icon: Icons.quiz_outlined, body: l.eduQuizzesInfo),
          OutlinedButton(
              onPressed: _openArticles, child: Text(l.eduAllArticles)),
        ] else
          AfyaUnavailableNote(
              icon: Icons.headphones_outlined,
              title: l.moduleUnavailableTitle,
              body: l.eduAudioUnavailable),
      ]),
    );
  }
}

// ---------------------------------------------------------------------------
// Panel 3 — HIV Support Chat (the existing anonymous Q&A)
// ---------------------------------------------------------------------------

/// Support landing over the existing anonymous Q&A. Category rows open the
/// existing question feed; the call-to-action opens the existing ask form.
/// Nothing here is live messaging, and the privacy card says so.
class SupportChatPage extends StatefulWidget {
  const SupportChatPage(
      {super.key,
      required this.nav,
      required this.repo,
      required this.familyProfilesCubit});
  final PatientShellNav nav;
  final QnaRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;
  @override
  State<SupportChatPage> createState() => _SupportChatPageState();
}

class _SupportChatPageState extends State<SupportChatPage> {
  late final QnaCubit _cubit = QnaCubit(widget.repo);

  @override
  void dispose() {
    _cubit.close();
    super.dispose();
  }

  void _openFeed() => Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => QnaPage(
          repo: widget.repo, familyProfilesCubit: widget.familyProfilesCubit)));

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    const green = [Color(0xFF22B886), Color(0xFF0C9A6C)];
    const teal = [Color(0xFF16A7A2), Color(0xFF0B8389)];
    const red = [Color(0xFFF4697A), Color(0xFFE23A50)];
    const purple = [Color(0xFFA176EA), Color(0xFF7C4FD2)];
    const indigo = [Color(0xFF6578EC), Color(0xFF3F51C9)];
    final cats = <(Widget, String, String)>[
      (
        const AfyaGradientIcon(colors: green, icon: Icons.science_rounded),
        l.supportCatTesting,
        l.supportCatTestingDesc
      ),
      (
        const AfyaGradientIcon(colors: teal, icon: Icons.medication_rounded),
        l.supportCatPrep,
        l.supportCatPrepDesc
      ),
      (
        const AfyaGradientIcon(colors: red, ribbon: true),
        l.supportCatArt,
        l.supportCatArtDesc
      ),
      (
        const AfyaGradientIcon(colors: purple, icon: Icons.favorite_rounded),
        l.supportCatSexualHealth,
        l.supportCatSexualHealthDesc
      ),
      (
        const AfyaGradientIcon(colors: indigo, icon: Icons.psychology_rounded),
        l.supportCatStigma,
        l.supportCatStigmaDesc
      ),
    ];
    return AfyaModuleScaffold(
      title: l.moduleSupportChat,
      nav: widget.nav,
      child: ListView(padding: _pagePadding, children: [
        Container(
          padding: const EdgeInsets.fromLTRB(14, 14, 14, 16),
          decoration: BoxDecoration(
              color: PatientTokens.cardTint,
              borderRadius: BorderRadius.circular(12)),
          child: Text('${l.supportIntro1}\n${l.supportIntro2}',
              style: const TextStyle(
                  fontSize: 15,
                  height: 1.55,
                  fontWeight: FontWeight.w700,
                  color: PatientTokens.introBlue)),
        ),
        const SizedBox(height: 16),
        for (final c in cats)
          Padding(
            padding: const EdgeInsets.only(bottom: 14),
            child: AfyaCard(
              onTap: _openFeed,
              padding: const EdgeInsets.fromLTRB(10, 10, 6, 10),
              child: Row(children: [
                c.$1,
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(c.$2,
                            style: const TextStyle(
                                fontSize: 14,
                                fontWeight: FontWeight.w700,
                                color: PatientTokens.navy)),
                        const SizedBox(height: 4),
                        Text(c.$3,
                            style: const TextStyle(
                                fontSize: 12.5, color: PatientTokens.ink)),
                      ]),
                ),
                const Icon(Icons.chevron_right_rounded,
                    color: PatientTokens.navy),
              ]),
            ),
          ),
        const SizedBox(height: 4),
        AfyaPillButton(
            label: l.supportAskAnonymous,
            icon: Icons.sms_rounded,
            color: PatientTokens.tealCta,
            onPressed: () => showAskQuestionSheet(context,
                cubit: _cubit,
                familyProfilesCubit: widget.familyProfilesCubit)),
        const SizedBox(height: 22),
        AfyaInfoCard(
            icon: Icons.lock_rounded,
            title: l.supportPrivacyTitle,
            body: l.supportPrivacyBody),
      ]),
    );
  }
}

// ---------------------------------------------------------------------------
// Panel 4 — Peer Navigator (no backend: layout only, action disabled)
// ---------------------------------------------------------------------------

class PeerNavigatorPage extends StatelessWidget {
  const PeerNavigatorPage({super.key, required this.nav});
  final PatientShellNav nav;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    Widget feature(IconData icon, Color c, Color bg, String t, String d) =>
        Padding(
          padding: const EdgeInsets.only(bottom: 12),
          child: AfyaCard(
            child: Row(children: [
              AfyaIconTile(icon: icon, color: c, background: bg, size: 46),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(t,
                          style: const TextStyle(
                              fontSize: 13.5,
                              fontWeight: FontWeight.w700,
                              color: PatientTokens.navy)),
                      const SizedBox(height: 3),
                      Text(d,
                          style: const TextStyle(
                              fontSize: 12.5, color: PatientTokens.ink)),
                    ]),
              ),
            ]),
          ),
        );
    return AfyaModuleScaffold(
      title: l.peerTitle,
      nav: nav,
      child: ListView(padding: _pagePadding, children: [
        const AfyaCharacterCard('peer_navigator_hero',
            background: PatientTokens.mintCard),
        const SizedBox(height: 14),
        Text(l.peerHeadline,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.w700,
                color: PatientTokens.navy)),
        const SizedBox(height: 8),
        Text(l.peerBody,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 14,
                height: 1.5,
                fontWeight: FontWeight.w600,
                color: PatientTokens.navy)),
        const SizedBox(height: 14),
        AfyaPillButton(
            label: l.peerStartChat, icon: Icons.sms_rounded, onPressed: null),
        const SizedBox(height: 8),
        Text(l.peerNotAvailable,
            textAlign: TextAlign.center,
            style:
                const TextStyle(fontSize: 12.5, color: PatientTokens.textSub)),
        const SizedBox(height: 16),
        feature(
            Icons.shield_outlined,
            PatientTokens.tileGreen,
            PatientTokens.tileGreenBg,
            l.peerFeatReminders,
            l.peerFeatRemindersDesc),
        feature(
            Icons.people_alt_rounded,
            PatientTokens.tileGreen,
            PatientTokens.tileGreenBg,
            l.peerFeatFollowUp,
            l.peerFeatFollowUpDesc),
        feature(
            Icons.groups_rounded,
            PatientTokens.tileSlateBlue,
            PatientTokens.tileSlateBlueBg,
            l.peerFeatLinkage,
            l.peerFeatLinkageDesc),
        feature(
            Icons.favorite_border_rounded,
            PatientTokens.tileRed,
            PatientTokens.tileRedBg,
            l.peerFeatEmotional,
            l.peerFeatEmotionalDesc),
        const SizedBox(height: 4),
        AfyaInfoCard(
            icon: Icons.phone_rounded,
            title: l.peerHereTitle,
            body: l.peerHereBody,
            background: PatientTokens.mintCard,
            accent: PatientTokens.primaryDark),
      ]),
    );
  }
}

// ---------------------------------------------------------------------------
// Panel 6 — Health Records (over the existing vault)
// ---------------------------------------------------------------------------

/// Health Records landing. Rows exist only for what the backend holds:
/// doctor-issued prescriptions and patient uploads. The panel's HIV Care
/// Summary, Lab Results and Appointments rows have no data source and are
/// not shown. Share Access has no backend and says so.
class HealthRecordsHubPage extends StatefulWidget {
  const HealthRecordsHubPage(
      {super.key,
      required this.nav,
      required this.repo,
      required this.familyProfilesCubit});
  final PatientShellNav nav;
  final VaultRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;
  @override
  State<HealthRecordsHubPage> createState() => _HealthRecordsHubPageState();
}

class _HealthRecordsHubPageState extends State<HealthRecordsHubPage> {
  int _tab = 0;

  void _open(String kind) => Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => HealthVaultPage(
          repo: widget.repo,
          familyProfilesCubit: widget.familyProfilesCubit,
          initialKind: kind)));

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    Widget row(IconData icon, Color c, Color bg, String t, String d,
            String kind) =>
        Padding(
          padding: const EdgeInsets.only(bottom: 14),
          child: AfyaCard(
            onTap: () => _open(kind),
            padding: const EdgeInsets.fromLTRB(12, 14, 6, 14),
            child: Row(children: [
              Container(
                  width: 52,
                  height: 52,
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                      color: bg, borderRadius: BorderRadius.circular(14)),
                  child: Icon(icon, color: c, size: 30)),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(t,
                          style: const TextStyle(
                              fontSize: 14,
                              fontWeight: FontWeight.w700,
                              color: PatientTokens.navy)),
                      const SizedBox(height: 4),
                      Text(d,
                          style: const TextStyle(
                              fontSize: 12.5, color: PatientTokens.ink)),
                    ]),
              ),
              const Icon(Icons.chevron_right_rounded,
                  color: PatientTokens.navy),
            ]),
          ),
        );
    return AfyaModuleScaffold(
      title: l.moduleHealthRecords,
      nav: widget.nav,
      child: ListView(padding: _pagePadding, children: [
        AfyaSegmentedTabs(
            expand: true,
            labels: [l.recordsTabMine, l.recordsTabShare],
            selected: _tab,
            onSelected: (i) => setState(() => _tab = i)),
        const SizedBox(height: 18),
        if (_tab == 0) ...[
          row(Icons.verified_user_outlined, PatientTokens.tileGreen,
              PatientTokens.tileGreenBg, l.recordsAll, l.recordsAllDesc, 'all'),
          row(
              Icons.event_note_rounded,
              PatientTokens.tileRed,
              PatientTokens.tileRedBg,
              l.recordsPrescriptions,
              l.recordsPrescriptionsDesc,
              'PRESCRIPTION'),
          row(
              Icons.description_rounded,
              PatientTokens.tealCta,
              PatientTokens.infoCard,
              l.recordsDocuments,
              l.recordsDocumentsDesc,
              'INTAKE'),
          const SizedBox(height: 4),
          AfyaInfoCard(
              icon: Icons.lock_rounded,
              title: l.recordsAccessTitle,
              body: '${l.recordsAccess1}\n${l.recordsAccess2}'),
        ] else
          AfyaUnavailableNote(
              icon: Icons.share_outlined,
              title: l.moduleUnavailableTitle,
              body: l.recordsShareUnavailable),
      ]),
    );
  }
}

// ---------------------------------------------------------------------------
// Panel 7 — Facilities & Services (no backend: chrome only, no listings)
// ---------------------------------------------------------------------------

/// Facilities layout without data. The panel's three hospitals, distances
/// and "Get Directions" links are sample content and are not reproduced
/// here. Search, filter chips and "View on Map" are shown disabled.
class FacilitiesPage extends StatelessWidget {
  const FacilitiesPage({super.key, required this.nav});
  final PatientShellNav nav;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AfyaModuleScaffold(
      title: l.moduleFacilities,
      nav: nav,
      child: ListView(padding: _pagePadding, children: [
        Container(
          constraints: const BoxConstraints(minHeight: 46),
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
          decoration: BoxDecoration(
              color: PatientTokens.cardTint,
              borderRadius: BorderRadius.circular(14)),
          child: Row(children: [
            const Icon(Icons.search_rounded,
                size: 22, color: PatientTokens.navInactive),
            const SizedBox(width: 10),
            Expanded(
                child: Text(l.facilitiesSearchHint,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 13, color: PatientTokens.hint))),
          ]),
        ),
        const SizedBox(height: 16),
        IgnorePointer(
          child: AfyaSegmentedTabs(labels: [
            l.facilitiesChipNearby,
            l.facilitiesChipTesting,
            l.facilitiesChipPrepArt,
            l.facilitiesChipYouth
          ], selected: 0, onSelected: (_) {}),
        ),
        AfyaUnavailableNote(
            icon: Icons.location_on_outlined,
            title: l.facilitiesEmptyTitle,
            body: l.facilitiesEmptyBody),
        const Opacity(
            opacity: .55,
            child: AfyaArt('facilities_map',
                height: 96, radius: 14, alignment: Alignment.center)),
        const SizedBox(height: 10),
        AfyaPillButton(
            label: l.facilitiesViewMap,
            icon: Icons.map_outlined,
            onPressed: null),
      ]),
    );
  }
}

// ---------------------------------------------------------------------------
// Panel 8 — Video Consultation landing (entry to the real booking flow)
// ---------------------------------------------------------------------------

/// Landing for Book Consultation. The panel's "Start Video Call" implies an
/// instant call; calls here are slot-booked and join-window gated, so the
/// button reads "Book a Video Consultation" and opens doctor discovery. The
/// urgent card dials the configured emergency number, not the panel's 11663.
class VideoConsultationPage extends StatelessWidget {
  const VideoConsultationPage(
      {super.key, required this.nav, required this.onFindDoctor});
  final PatientShellNav nav;
  final VoidCallback onFindDoctor;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AfyaModuleScaffold(
      title: l.consultTitle,
      nav: nav,
      child: ListView(padding: _pagePadding, children: [
        const AfyaCharacterCard('doctor_consult_hero',
            background: PatientTokens.doctorCard,
            panel: PatientTokens.doctorPanel),
        const SizedBox(height: 18),
        Text(l.consultHeadline,
            style: const TextStyle(
                fontSize: 19,
                fontWeight: FontWeight.w800,
                color: PatientTokens.navy)),
        const SizedBox(height: 8),
        Text(l.consultBody,
            style: const TextStyle(
                fontSize: 15, height: 1.5, color: PatientTokens.ink)),
        const SizedBox(height: 10),
        AfyaCheckRow(l.consultCheck1),
        AfyaCheckRow(l.consultCheck2),
        AfyaCheckRow(l.consultCheck3),
        const SizedBox(height: 14),
        AfyaPillButton(
            label: l.consultCta,
            icon: Icons.videocam_rounded,
            onPressed: onFindDoctor),
        const _MyUpcomingConsultation(),
        const SizedBox(height: 18),
        AfyaCard(
          color: PatientTokens.urgentCard,
          shadow: false,
          onTap: () => launchUrl(Uri.parse(EmergencyContact.kenyaTelUri)),
          padding: const EdgeInsets.fromLTRB(16, 18, 16, 18),
          child: Row(children: [
            const AfyaIconTile(
                icon: Icons.phone_rounded,
                color: Colors.white,
                background: PatientTokens.urgentIcon,
                size: 46),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(l.consultUrgentTitle,
                        style: const TextStyle(
                            fontSize: 14,
                            fontWeight: FontWeight.w700,
                            color: PatientTokens.urgentIcon)),
                    const SizedBox(height: 4),
                    Text(l.consultUrgentBody(EmergencyContact.kenyaNumber),
                        style: const TextStyle(
                            fontSize: 13.5,
                            height: 1.45,
                            color: PatientTokens.ink)),
                  ]),
            ),
          ]),
        ),
      ]),
    );
  }
}

/// The patient's nearest upcoming consultation and its waiting-room entry.
/// Renders nothing when there is none, so the landing matches the reference.
class _MyUpcomingConsultation extends StatelessWidget {
  const _MyUpcomingConsultation();
  @override
  Widget build(BuildContext context) =>
      BlocBuilder<HomeCubit, HomeState>(builder: (context, state) {
        final next = state.nextUpcoming;
        if (next == null) return const SizedBox.shrink();
        return Padding(
          padding: const EdgeInsets.only(top: 8),
          child: UpcomingConsultationSection(
              state: state, onManage: () => openWaitingRoomFor(context, next)),
        );
      });
}

// ---------------------------------------------------------------------------
// Panel 11 — Privacy & Security
// ---------------------------------------------------------------------------

/// Privacy information page. Each line is limited to what the app does
/// today; the panel's "Anonymous questions and chat" is narrowed to
/// questions, because consultation chat is not anonymous.
class PrivacySecurityPage extends StatelessWidget {
  const PrivacySecurityPage({super.key, required this.nav});
  final PatientShellNav nav;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AfyaModuleScaffold(
      title: l.privacyTitle,
      nav: nav,
      selectedTab: 3,
      child: ListView(padding: _pagePadding, children: [
        Container(
          padding: const EdgeInsets.fromLTRB(16, 18, 16, 20),
          decoration: BoxDecoration(
              color: PatientTokens.mintCard.withValues(alpha: .6),
              borderRadius: BorderRadius.circular(14)),
          child: Column(children: [
            const AfyaIconTile(
                icon: Icons.lock_rounded,
                color: Colors.white,
                background: PatientTokens.tealCta,
                size: 46),
            const SizedBox(height: 14),
            Text(l.privacyHeadline,
                textAlign: TextAlign.center,
                style: const TextStyle(
                    fontSize: 19,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.navy)),
          ]),
        ),
        const SizedBox(height: 12),
        AfyaCheckRow(l.privacyCheck1),
        AfyaCheckRow(l.privacyCheck2),
        AfyaCheckRow(l.privacyCheck3),
        AfyaCheckRow(l.privacyCheck4),
        AfyaCheckRow(l.privacyCheck5),
        AfyaCheckRow(l.privacyCheck6),
        const SizedBox(height: 16),
        AfyaInfoCard(
            icon: Icons.verified_user_outlined,
            title: l.privacyChoiceTitle,
            body: l.privacyChoiceBody),
        const SizedBox(height: 16),
        OutlinedButton(
            onPressed: () => Navigator.of(context).push(
                MaterialPageRoute(builder: (_) => const DataRightsPage())),
            child: Text(l.privacyDataRights)),
      ]),
    );
  }
}
