/// WhatsApp contact shown on the HIV Support Chat screen ("Need immediate
/// assistance? Text us on WhatsApp"). Product-owner instruction 2026-11-14:
/// the number is not supplied yet, so the screen shows a placeholder.
///
/// Set it at build time, for example
/// `--dart-define=SUPPORT_WHATSAPP_NUMBER=+254700000000`. While it is empty
/// the card states that the number is coming soon and has no tap action;
/// a number is never invented here.
abstract final class SupportWhatsApp {
  static const String number =
      String.fromEnvironment('SUPPORT_WHATSAPP_NUMBER');
}
