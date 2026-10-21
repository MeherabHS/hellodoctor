/// The old Kenya verification walkthrough put its nonce and review action in
/// full_name. Match only that proven fixture shape; never rewrite real names.
String patientDoctorName(String name) => name.replaceFirst(
      RegExp(
          r'^(Dr\. Synthetic Kenya Review) [a-f0-9]{10} (?:Approve|Reject)$'),
      'Dr. Synthetic Kenya Review',
    );
