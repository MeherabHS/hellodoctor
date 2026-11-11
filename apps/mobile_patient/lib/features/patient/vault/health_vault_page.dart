// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:intl/intl.dart';

import '../../../core/network/api_client.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../profile/family_profiles_cubit.dart';
import 'clinical_pdf_viewer.dart';
import 'vault_cubit.dart';
import 'vault_models.dart';
import 'vault_repository.dart';

/// `prototype/index.html` view-5 — "Health Vault". Tabs are relabeled from
/// the prototype's "All Records / e-Prescriptions / Lab Reports / Doctor
/// Notes" to "All / e-Prescriptions / My Uploads": the backend has exactly
/// two document kinds (`PRESCRIPTION`, doctor-issued; `INTAKE`,
/// patient-uploaded) — it does not distinguish a lab report from a doctor's
/// note within an intake upload, so inventing that split client-side would
/// be a fabricated category, not a real filter.
class HealthVaultPage extends StatefulWidget {
  const HealthVaultPage({
    super.key,
    required this.repo,
    required this.familyProfilesCubit,
  });
  final VaultRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;

  @override
  State<HealthVaultPage> createState() => _HealthVaultPageState();
}

class _HealthVaultPageState extends State<HealthVaultPage> {
  late final VaultCubit _cubit;
  String _kindFilter = 'all';
  final Set<String> _openingIds = {};

  @override
  void initState() {
    super.initState();
    _cubit = VaultCubit(widget.repo, connectivity: widget.repo.connectivity)
      ..setActiveProfile(widget.familyProfilesCubit.state.activeProfileId)
      ..load();
  }

  @override
  void dispose() {
    _cubit.close();
    super.dispose();
  }

  /// Remediation E2: fetches the document's bytes and cross-checks the
  /// server's own authoritative `Content-Type` response header against the
  /// `mimeType` declared in the document listing. A mismatch means the
  /// listing and the actual stored object disagree about what this file is —
  /// refuse to render either way rather than trusting either source alone.
  Future<Uint8List> _loadVerifiedBytes(PatientDocumentDto doc) async {
    final result =
        await widget.repo.fetchDocumentBytes(doc.appointmentId, doc.id);
    final declared = doc.mimeType;
    if (declared != null &&
        result.contentType != null &&
        declared.toLowerCase() != result.contentType) {
      throw const ApiException(
          409, 'This document could not be verified and was not opened.');
    }
    return result.bytes;
  }

  Future<void> _open(PatientDocumentDto doc) async {
    if (_openingIds.contains(doc.id)) return;
    setState(() => _openingIds.add(doc.id));
    try {
      final mime = doc.mimeType?.toLowerCase();
      if (mime == 'image/jpeg' || mime == 'image/png') {
        final url =
            await widget.repo.resolveDownloadUrl(doc.appointmentId, doc.id);
        if (!mounted) return;
        await Navigator.of(context).push(MaterialPageRoute(
          builder: (_) =>
              _ImageViewerPage(url: url, title: _titleOf(context, doc)),
        ));
      } else if (mime == 'application/pdf') {
        if (!mounted) return;
        await Navigator.of(context).push(MaterialPageRoute(
          builder: (_) => ClinicalPdfViewer(
              load: () => _loadVerifiedBytes(doc),
              title: _titleOf(context, doc)),
        ));
      } else if (mounted) {
        // No external-application fallback for an unrecognized or missing
        // mime type — a safe "cannot preview" state, never a guess.
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(
            content: Text(AppLocalizations.of(context).vaultOpenError)));
      }
    } catch (_) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(
          content: Text(AppLocalizations.of(context).vaultOpenNowError)));
    } finally {
      if (mounted) setState(() => _openingIds.remove(doc.id));
    }
  }

  String _titleOf(BuildContext context, PatientDocumentDto doc) => doc
          .isPrescription
      ? AppLocalizations.of(context).vaultPrescriptionTitle(doc.rxNumber ?? '')
      : AppLocalizations.of(context).vaultUploadedDocument;

  @override
  Widget build(BuildContext context) {
    return BlocProvider.value(
      value: _cubit,
      child: Scaffold(
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
          child: Column(children: [
            PatientPageHeader(AppLocalizations.of(context).vaultTitle,
                AppLocalizations.of(context).vaultSubtitle),
            BlocListener<FamilyProfilesCubit, FamilyProfilesState>(
              bloc: widget.familyProfilesCubit,
              listenWhen: (previous, current) =>
                  previous.activeProfileId != current.activeProfileId,
              listener: (context, state) =>
                  _cubit.setActiveProfile(state.activeProfileId),
              child: const SizedBox.shrink(),
            ),
            _KindTabs(
              selected: _kindFilter,
              onSelected: (k) => setState(() => _kindFilter = k),
            ),
            Expanded(
              child: BlocBuilder<VaultCubit, VaultState>(
                builder: (context, state) {
                  return RefreshIndicator(
                    onRefresh: _cubit.load,
                    child: _VaultList(
                      state: state,
                      kindFilter: _kindFilter,
                      openingIds: _openingIds,
                      onOpen: _open,
                    ),
                  );
                },
              ),
            ),
          ]),
        ),
      ),
    );
  }
}

class _KindTabs extends StatelessWidget {
  const _KindTabs({required this.selected, required this.onSelected});
  final String selected;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final tabs = [
      ('all', l10n.vaultAllRecords),
      ('PRESCRIPTION', l10n.vaultPrescriptions),
      ('INTAKE', l10n.vaultUploads),
    ];
    return Container(
      color: PatientTokens.surface,
      padding: const EdgeInsets.fromLTRB(18, 10, 18, 10),
      child: Row(children: [
        for (final t in tabs)
          Padding(
            padding: const EdgeInsets.only(right: 8),
            child: ChoiceChip(
              label: Text(t.$2),
              selected: selected == t.$1,
              onSelected: (_) => onSelected(t.$1),
              selectedColor: PatientTokens.primaryLight,
              labelStyle: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w700,
                  color: selected == t.$1
                      ? PatientTokens.primaryDark
                      : PatientTokens.text),
              side: const BorderSide(color: PatientTokens.border),
            ),
          ),
      ]),
    );
  }
}

class _VaultList extends StatelessWidget {
  const _VaultList({
    required this.state,
    required this.kindFilter,
    required this.openingIds,
    required this.onOpen,
  });
  final VaultState state;
  final String kindFilter;
  final Set<String> openingIds;
  final void Function(PatientDocumentDto) onOpen;

  @override
  Widget build(BuildContext context) {
    if (state.loading && state.documents.isEmpty) {
      return ListView(
        padding: const EdgeInsets.all(18),
        children: const [
          SkeletonBlock(height: 70),
          SizedBox(height: 10),
          SkeletonBlock(height: 70),
        ],
      );
    }
    if (state.error != null && state.documents.isEmpty) {
      return ErrorPanel(state.error!);
    }
    final visible = kindFilter == 'all'
        ? state.documents
        : state.documents.where((d) => d.kind == kindFilter).toList();
    if (visible.isEmpty) {
      return ListView(
        children: [
          Padding(
            padding: EdgeInsets.symmetric(vertical: 60),
            child: Column(children: [
              Icon(Icons.folder_off_outlined,
                  color: PatientTokens.textSub, size: 32),
              SizedBox(height: 8),
              Text(AppLocalizations.of(context).vaultEmpty,
                  style: TextStyle(
                      fontWeight: FontWeight.w700, color: PatientTokens.text)),
              SizedBox(height: 4),
              Text(AppLocalizations.of(context).vaultEmptyHint,
                  style: TextStyle(fontSize: 11, color: PatientTokens.textSub)),
            ]),
          ),
        ],
      );
    }

    final groups = <String, List<PatientDocumentDto>>{};
    for (final doc in visible) {
      final key = DateFormat.yMMMM(AppLocalizations.of(context).localeName)
          .format(doc.createdAt);
      groups.putIfAbsent(key, () => []).add(doc);
    }

    return ListView(
      padding: const EdgeInsets.fromLTRB(18, 10, 18, 24),
      children: [
        if (state.offline)
          const CacheStatusBar(offline: true, refreshing: false),
        for (final entry in groups.entries) ...[
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8),
            child: Text(entry.key,
                style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.textSub)),
          ),
          for (final doc in entry.value)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: _DocumentTile(
                doc: doc,
                opening: openingIds.contains(doc.id),
                onTap: () => onOpen(doc),
              ),
            ),
        ],
      ],
    );
  }
}

class _DocumentTile extends StatelessWidget {
  const _DocumentTile(
      {required this.doc, required this.opening, required this.onTap});
  final PatientDocumentDto doc;
  final bool opening;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final isRx = doc.isPrescription;
    return InkWell(
      onTap: opening ? null : onTap,
      borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
      child: PatientCard(
        child: Row(children: [
          Container(
            width: 42,
            height: 42,
            decoration: BoxDecoration(
                color: isRx
                    ? PatientTokens.primaryLight
                    : PatientTokens.surfaceAlt,
                borderRadius: BorderRadius.circular(10)),
            alignment: Alignment.center,
            child: Icon(
                isRx ? Icons.medication_outlined : Icons.description_outlined,
                color:
                    isRx ? PatientTokens.primaryDark : PatientTokens.primary),
          ),
          const SizedBox(width: 12),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(
                  isRx
                      ? AppLocalizations.of(context)
                          .vaultPrescriptionTitle(doc.rxNumber ?? '')
                      : doc.pageNumber == null
                          ? AppLocalizations.of(context).vaultUploadedDocument
                          : AppLocalizations.of(context)
                              .vaultUploadedPage(doc.pageNumber.toString()),
                  style: const TextStyle(
                      fontWeight: FontWeight.w700, color: PatientTokens.text)),
              Text(
                  isRx
                      ? '${doc.doctorDisplayName ?? ''} • ${doc.primarySpecialty ?? ''}'
                      : AppLocalizations.of(context).vaultUploadSource,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontSize: 11.5, color: PatientTokens.textSub)),
            ]),
          ),
          if (opening)
            const SizedBox(
                width: 16,
                height: 16,
                child: CircularProgressIndicator(strokeWidth: 2))
          else
            StatusChip(
                isRx
                    ? AppLocalizations.of(context).vaultPrescriptionBadge
                    : AppLocalizations.of(context).vaultUploadBadge,
                color: isRx
                    ? PatientTokens.primaryLight
                    : PatientTokens.surfaceAlt,
                textColor:
                    isRx ? PatientTokens.primaryDark : PatientTokens.primary),
        ]),
      ),
    );
  }
}

class _ImageViewerPage extends StatelessWidget {
  const _ImageViewerPage({required this.url, required this.title});
  final Uri url;
  final String title;

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: Colors.black,
        appBar: AppBar(
          backgroundColor: Colors.black,
          foregroundColor: Colors.white,
          title: Text(title),
        ),
        body: Center(
          child: InteractiveViewer(
            child: Image.network(
              url.toString(),
              errorBuilder: (context, error, stack) => Padding(
                padding: EdgeInsets.all(24),
                child: Text(AppLocalizations.of(context).vaultImageError,
                    style: TextStyle(color: Colors.white)),
              ),
            ),
          ),
        ),
      );
}
