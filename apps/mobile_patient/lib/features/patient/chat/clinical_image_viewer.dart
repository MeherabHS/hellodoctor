import 'dart:typed_data';
import 'package:flutter/material.dart';

/// Bytes exist only in this route's memory, never in a generic image disk cache.
class ClinicalImageViewer extends StatefulWidget {
  const ClinicalImageViewer({super.key, required this.load});
  final Future<Uint8List> Function() load;
  @override
  State<ClinicalImageViewer> createState() => _ClinicalImageViewerState();
}

class _ClinicalImageViewerState extends State<ClinicalImageViewer> {
  late Future<Uint8List> _image;
  MemoryImage? _provider;
  @override
  void dispose() {
    _provider?.evict();
    _provider = null;
    super.dispose();
  }

  @override
  void initState() {
    super.initState();
    _image = widget.load();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('Clinical image')),
      body: SafeArea(
          child: FutureBuilder<Uint8List>(
              future: _image,
              builder: (context, snapshot) {
                if (snapshot.hasError) {
                  return Center(
                      child: Column(mainAxisSize: MainAxisSize.min, children: [
                    const Text('Unable to open this image.'),
                    TextButton(
                        onPressed: () => setState(() => _image = widget.load()),
                        child: const Text('Retry'))
                  ]));
                }
                if (!snapshot.hasData) {
                  return const Center(child: CircularProgressIndicator());
                }
                if (_provider?.bytes != snapshot.data) {
                  _provider?.evict();
                  _provider = MemoryImage(snapshot.data!);
                }
                return InteractiveViewer(
                    child: Center(
                        child:
                            Image(image: _provider!, gaplessPlayback: false)));
              })));
}
