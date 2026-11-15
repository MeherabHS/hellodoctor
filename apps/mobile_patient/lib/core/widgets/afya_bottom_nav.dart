import 'package:flutter/material.dart';

import '../../l10n/app_localizations.dart';
import '../theme/tokens.dart';

/// Bottom navigation of the reference: Home, Services, Messages, Profile.
/// The active item uses a filled icon and the deep brand green; inactive
/// items are slate outlines. No indicator pill and no top border.
class PatientBottomNav extends StatelessWidget {
  const PatientBottomNav(
      {super.key, required this.selected, required this.onSelected});
  final int selected;
  final ValueChanged<int> onSelected;
  static const items = <(IconData, IconData)>[
    (Icons.home_outlined, Icons.home_rounded),
    (Icons.grid_view_outlined, Icons.grid_view_rounded),
    (Icons.chat_bubble_outline_rounded, Icons.chat_bubble_rounded),
    (Icons.person_outline_rounded, Icons.person_rounded),
  ];
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final labels = [
      l10n.navHome,
      l10n.navServices,
      l10n.navMessages,
      l10n.navProfile,
    ];
    return Container(
        height: PatientTokens.navHeight,
        decoration: const BoxDecoration(color: Colors.white, boxShadow: [
          BoxShadow(
              color: Color(0x0F102A43), blurRadius: 12, offset: Offset(0, -2))
        ]),
        child: Material(
          color: Colors.transparent,
          child: Row(children: [
            for (var i = 0; i < items.length; i++)
              Expanded(
                  child: InkWell(
                      onTap: () => onSelected(i),
                      child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Icon(selected == i ? items[i].$2 : items[i].$1,
                                size: 25,
                                color: selected == i
                                    ? PatientTokens.navActive
                                    : PatientTokens.navInactive),
                            const SizedBox(height: 4),
                            Text(labels[i],
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                textScaler: MediaQuery.textScalerOf(context)
                                    .clamp(maxScaleFactor: 1.3),
                                style: TextStyle(
                                    fontSize: 11.5,
                                    fontWeight: selected == i
                                        ? FontWeight.w700
                                        : FontWeight.w500,
                                    color: selected == i
                                        ? PatientTokens.navActive
                                        : PatientTokens.navInactive))
                          ])))
          ]),
        ));
  }
}
