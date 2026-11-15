import 'dart:async';

import 'package:flutter/material.dart';

import '../../../l10n/app_localizations.dart';
import '../home/brand_header.dart';
import '../modules/afya_module_widgets.dart';
import '../modules/patient_modules.dart';
import 'facility_details_page.dart';
import 'facility_models.dart';
import 'facility_repository.dart';
import 'facility_widgets.dart';

/// Facility Services directory (reference: left and middle panels of
/// `docs/design/facilities/facility-services-flow.png`). Lists only facilities
/// an Admin has published. No location is requested or used (product owner,
/// 2026-11-13: no Nearby filter); each facility's Get Directions opens the
/// Admin's Google Maps link.
class FacilityDirectoryPage extends StatefulWidget {
  const FacilityDirectoryPage(
      {super.key, required this.repo, required this.nav, this.appHeader});
  final FacilityRepository repo;
  final PatientShellNav nav;
  final AppHeaderDeps? appHeader;
  @override
  State<FacilityDirectoryPage> createState() => _FacilityDirectoryPageState();
}

class _FacilityDirectoryPageState extends State<FacilityDirectoryPage>
    with WidgetsBindingObserver {
  final _scroll = ScrollController();
  final _searchController = TextEditingController();
  Timer? _debounce, _statusTimer;
  var _filter = const FacilityFilter();
  final _items = <FacilityDto>[];
  int? _nextOffset;
  bool _loading = false, _failed = false, _loaded = false, _loadingMore = false;
  int _generation = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _scroll.addListener(() {
      if (_scroll.position.extentAfter < 300) _loadMore();
    });
    _reload();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _debounce?.cancel();
    _statusTimer?.cancel();
    _scroll.dispose();
    _searchController.dispose();
    super.dispose();
  }

  /// Opening status is computed by the server; refresh on return to the app.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed && _loaded) _reload(quiet: true);
  }

  Future<void> _reload({bool quiet = false}) async {
    final generation = ++_generation;
    setState(() {
      _failed = false;
      if (!quiet) _loading = true;
    });
    try {
      final page = await widget.repo.list(_filter);
      if (!mounted || generation != _generation) return;
      setState(() {
        _items
          ..clear()
          ..addAll(page.items);
        _nextOffset = page.nextOffset;
        _loaded = true;
      });
      _scheduleStatusRefresh();
    } catch (_) {
      if (!mounted || generation != _generation) return;
      setState(() => _failed = true);
    } finally {
      if (mounted && generation == _generation) {
        setState(() => _loading = false);
      }
    }
  }

  Future<void> _loadMore() async {
    final offset = _nextOffset;
    if (offset == null || _loadingMore || _loading) return;
    final generation = _generation;
    setState(() => _loadingMore = true);
    try {
      final page = await widget.repo.list(_filter, offset: offset);
      if (!mounted || generation != _generation) return;
      setState(() {
        _items.addAll(page.items);
        _nextOffset = page.nextOffset;
      });
    } catch (_) {
      // The list stays usable; scrolling again retries.
    } finally {
      if (mounted) setState(() => _loadingMore = false);
    }
  }

  /// Re-fetch when the earliest listed opening or closing time passes, so an
  /// open screen does not keep showing a stale "Open".
  void _scheduleStatusRefresh() {
    _statusTimer?.cancel();
    final now = DateTime.now().toUtc();
    final next = _items
        .map((f) => f.status.nextChangeAt)
        .whereType<DateTime>()
        .where((t) => t.isAfter(now))
        .fold<DateTime?>(null, (a, b) => a == null || b.isBefore(a) ? b : a);
    if (next == null) return;
    var wait = next.difference(now) + const Duration(seconds: 5);
    if (wait > const Duration(hours: 1)) wait = const Duration(hours: 1);
    _statusTimer = Timer(wait, () {
      if (mounted) _reload(quiet: true);
    });
  }

  void _setFilter(FacilityFilter next) {
    _filter = next;
    _reload();
  }

  void _onSearch(String value) {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 350), () {
      if (mounted) _setFilter(_filter.copyWith(search: value));
    });
  }

  void _open(FacilityDto f) => Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => FacilityDetailsPage(
          facility: f, repo: widget.repo, nav: widget.nav)));

  Widget _chip(String label, bool on, VoidCallback onTap, {IconData? icon}) =>
      Padding(
        padding: const EdgeInsets.only(right: 8),
        child: Semantics(
          button: true,
          selected: on,
          child: Material(
            color: on ? FacColors.chipOn : Colors.white,
            shape: StadiumBorder(
                side: BorderSide(
                    color: on ? FacColors.chipOn : FacColors.divider)),
            elevation: on ? 0 : 1,
            shadowColor: const Color(0x22000000),
            child: InkWell(
              customBorder: const StadiumBorder(),
              onTap: onTap,
              child: Padding(
                padding:
                    const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  if (icon != null) ...[
                    Icon(icon,
                        size: 16, color: on ? Colors.white : FacColors.ink),
                    const SizedBox(width: 4),
                  ],
                  Text(label,
                      style: TextStyle(
                          fontSize: 13.5,
                          fontWeight: FontWeight.w600,
                          color: on ? Colors.white : FacColors.ink)),
                ]),
              ),
            ),
          ),
        ),
      );

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AfyaModuleScaffold(
      title: l.facilitiesTitle,
      nav: widget.nav,
      appHeader: widget.appHeader,
      child: RefreshIndicator(
        onRefresh: () => _reload(quiet: true),
        child: ListView(
          controller: _scroll,
          physics: const AlwaysScrollableScrollPhysics(),
          padding: const EdgeInsets.fromLTRB(20, 0, 20, 24),
          children: [
            Text(l.facilitiesSubtitle,
                style: const TextStyle(
                    fontSize: 14, height: 1.4, color: FacColors.body)),
            const SizedBox(height: 14),
            TextField(
              controller: _searchController,
              onChanged: _onSearch,
              textInputAction: TextInputAction.search,
              maxLength: 100,
              decoration: InputDecoration(
                counterText: '',
                hintText: l.facilitiesSearchHint,
                prefixIcon: const Icon(Icons.search_rounded),
                filled: true,
                fillColor: FacColors.field,
                contentPadding: const EdgeInsets.symmetric(vertical: 14),
                border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(28),
                    borderSide: BorderSide.none),
              ),
            ),
            const SizedBox(height: 14),
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(children: [
                _chip(
                    l.facilitiesChipTesting,
                    _filter.testing,
                    () => _setFilter(
                        _filter.copyWith(testing: !_filter.testing))),
                _chip(
                    l.facilitiesChipPrepArt,
                    _filter.prepArt,
                    () => _setFilter(
                        _filter.copyWith(prepArt: !_filter.prepArt))),
                _chip(
                    l.facilitiesChipYouth,
                    _filter.youthFriendly,
                    () => _setFilter(_filter.copyWith(
                        youthFriendly: !_filter.youthFriendly))),
              ]),
            ),
            const SizedBox(height: 8),
            if (_loading && _items.isEmpty)
              for (var i = 0; i < 3; i++) const _SkeletonRow()
            else if (_failed && _items.isEmpty)
              _ErrorPanel(onRetry: _reload)
            else if (_items.isEmpty)
              FacilityEmptyState(filtered: _filter.isFiltered)
            else ...[
              for (final f in _items)
                FacilityCard(
                    facility: f, repo: widget.repo, onOpen: () => _open(f)),
              if (_loadingMore)
                const Padding(
                    padding: EdgeInsets.all(16),
                    child: Center(child: CircularProgressIndicator())),
            ],
            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }
}

class _SkeletonRow extends StatelessWidget {
  const _SkeletonRow();
  @override
  Widget build(BuildContext context) {
    Widget bar(double w, double h) => Container(
        width: w,
        height: h,
        decoration: BoxDecoration(
            color: const Color(0xFFEDF1F4),
            borderRadius: BorderRadius.circular(6)));
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 12),
      child: Row(children: [
        bar(92, 82),
        const SizedBox(width: 14),
        Expanded(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            bar(160, 14),
            const SizedBox(height: 8),
            bar(110, 12),
            const SizedBox(height: 8),
            bar(130, 12),
          ]),
        ),
      ]),
    );
  }
}

class _ErrorPanel extends StatelessWidget {
  const _ErrorPanel({required this.onRetry});
  final VoidCallback onRetry;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 28),
      child: Column(children: [
        const Icon(Icons.wifi_off_rounded, size: 36, color: FacColors.muted),
        const SizedBox(height: 10),
        Text(l.facilitiesLoadError,
            textAlign: TextAlign.center,
            style: const TextStyle(color: FacColors.body)),
        const SizedBox(height: 10),
        OutlinedButton(onPressed: onRetry, child: Text(l.facilitiesRetry)),
      ]),
    );
  }
}
