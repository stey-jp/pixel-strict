import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';
import 'dart:typed_data';

import 'native_bindings.dart' as native;

const maxInputBytes = 64 * 1024 * 1024;

class ConversionSettings {
  const ConversionSettings({
    this.gridWidth,
    this.colors,
    this.smoothing = 2,
    this.edgeProtection = 2,
    this.median = false,
  });
  final int? gridWidth;
  final int? colors;
  final int smoothing;
  final int edgeProtection;
  final bool median;
  Map<String, Object?> toJson() => {
    'grid_width': gridWidth,
    'colors': colors,
    'smoothing': smoothing,
    'edge_protection': edgeProtection,
    'median': median,
  };
}

class ConversionResult {
  const ConversionResult(this.png, this.report);
  final Uint8List png;
  final Map<String, dynamic> report;
  int get width => (report['grid'] as Map)['width'] as int;
  int get height => (report['grid'] as Map)['height'] as int;
  int get colorCount => (report['palette'] as List).length;
  double get milliseconds => (report['processing_ms'] as num).toDouble();
}

Future<ConversionResult> convertImage(
  Uint8List bytes,
  ConversionSettings settings,
) async {
  if (bytes.isEmpty || bytes.length > maxInputBytes) {
    throw const FormatException('画像は1 byte〜64 MiBで指定してください。');
  }
  final transferred = TransferableTypedData.fromList([bytes]);
  final json = jsonEncode(settings.toJson());
  return Isolate.run(
    () => _convert(transferred.materialize().asUint8List(), json),
  );
}

ConversionResult _convert(Uint8List bytes, String settings) {
  final json = utf8.encode(settings);
  final input = native.allocate(bytes.length);
  final options = native.allocate(json.length);
  Pointer<Void> result = nullptr;
  try {
    if (input == nullptr || options == nullptr) {
      throw StateError('画像バッファを確保できません。');
    }
    input.asTypedList(bytes.length).setAll(0, bytes);
    options.asTypedList(json.length).setAll(0, json);
    result = native.convertNative(input, bytes.length, options, json.length);
    if (result == nullptr) throw StateError('変換結果を取得できません。');
    final report = jsonDecode(
      utf8.decode(
        native.resultJson(result).asTypedList(native.resultJsonLength(result)),
      ),
    ) as Map<String, dynamic>;
    if (report.containsKey('error')) {
      throw FormatException(report['error'] as String);
    }
    final png = Uint8List.fromList(
      native.resultPng(result).asTypedList(native.resultPngLength(result)),
    );
    return ConversionResult(png, report);
  } finally {
    if (result != nullptr) native.freeResult(result);
    native.deallocate(input, bytes.length);
    native.deallocate(options, json.length);
  }
}
