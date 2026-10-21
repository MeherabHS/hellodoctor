import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/network/api_client.dart';
import '../../auth/auth_cubit.dart';

class DataRightsPage extends StatefulWidget {
  const DataRightsPage({super.key});
  @override
  State<DataRightsPage> createState() => _DataRightsPageState();
}

class _DataRightsPageState extends State<DataRightsPage> {
  String type = 'ACCESS';
  final details = TextEditingController();
  List<Map<String, dynamic>> requests = [];
  String? error;
  bool busy = false;
  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    details.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final r =
          await context.read<AuthCubit>().api.get('/patients/me/data-requests');
      if (mounted) {
        setState(() => requests = (r.data['data'] as List)
            .whereType<Map>()
            .map((e) => Map<String, dynamic>.from(e))
            .toList());
      }
    } on ApiException catch (e) {
      if (mounted) setState(() => error = e.userMessage);
    }
  }

  Future<void> _submit() async {
    if (details.text.trim().isEmpty) return;
    setState(() => busy = true);
    try {
      await context.read<AuthCubit>().api.post('/patients/me/data-requests',
          data: {'request_type': type, 'details': details.text.trim()});
      details.clear();
      await _load();
    } on ApiException catch (e) {
      if (mounted) setState(() => error = e.userMessage);
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('Legal & your data')),
      body: ListView(padding: const EdgeInsets.all(20), children: [
        const Text(
            'Request access, correction, deletion, withdrawal or a privacy review. Requests are assessed under Kenyan law and medical-record retention rules.'),
        const SizedBox(height: 18),
        DropdownButtonFormField<String>(
            initialValue: type,
            items: const [
              'ACCESS',
              'CORRECTION',
              'DELETION',
              'WITHDRAWAL',
              'RESTRICTION',
              'PORTABILITY',
              'COMPLAINT'
            ].map((v) => DropdownMenuItem(value: v, child: Text(v))).toList(),
            onChanged: (v) {
              if (v != null) setState(() => type = v);
            },
            decoration: const InputDecoration(labelText: 'Request type')),
        const SizedBox(height: 12),
        TextField(
            controller: details,
            maxLines: 5,
            maxLength: 2000,
            decoration: const InputDecoration(labelText: 'What do you need?')),
        const SizedBox(height: 12),
        FilledButton(
            onPressed: busy ? null : _submit,
            child: Text(busy ? 'Submitting…' : 'Submit request')),
        if (error != null)
          Text(error!, style: const TextStyle(color: Colors.red)),
        const SizedBox(height: 24),
        const Text('Your requests',
            style: TextStyle(fontWeight: FontWeight.bold)),
        ...requests.map((r) => ListTile(
            title: Text('${r['request_type']} · ${r['status']}'),
            subtitle: Text(r['details'].toString()),
            isThreeLine: true))
      ]));
}
