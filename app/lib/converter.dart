import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';
import 'dart:typed_data';

import 'native_bindings.dart' as native;

const maxInputBytes = 64 * 1024 * 1024;

enum OutputMode { preserve, logical }

// Only these localized messages are shown verbatim by the UI.
class ImageConversionException extends FormatException {
  const ImageConversionException(super.message);
}

String _nativeErrorMessage(String message) {
  if (message.startsWith('Preserve output requires')) {
    return 'このGrid幅では元のサイズと均等な正方形セルを両立できません。'
        '両辺を割り切るセル幅にするか、Logical pixelsを選んでください。';
  }
  if (message.startsWith('No valid grid candidates')) {
    return '元のサイズを保てるグリッドがセル数上限内にありません。'
        'Logical pixelsを選んでください。';
  }
  if (message.startsWith('Output exceeds')) {
    final limit = RegExp(r'\d+').firstMatch(message)?.group(0);
    return 'セル数が上限${limit == null ? '' : '（$limit）'}を超えています。'
        'Grid幅を小さくしてください。';
  }
  if (message.startsWith('Not enough memory')) {
    return 'メモリが不足しています。Grid幅を小さくするか、小さい画像を選んでください。';
  }
  if (message.startsWith('Grid width must')) {
    return 'Grid幅は元画像の幅以下の正の整数で指定してください。';
  }
  if (message.startsWith('Colors must')) return '色数はAuto・16・24・32から選んでください。';
  if (message.startsWith('Strength must')) return '各処理の強さは1〜3で指定してください。';
  if (message.startsWith('Image limit:')) return '画像上限は各辺8192px・合計16メガピクセルです。';
  if (message.startsWith('Input must')) return '画像は1 byte〜64 MiBで指定してください。';
  if (message.startsWith('Supported formats:')) {
    return 'PNG・JPEG・WebP画像を選んでください。';
  }
  if (message.startsWith('Invalid image:') ||
      message.startsWith('Decode failed:')) {
    return '画像を読み込めません。ファイルが壊れていないか確認してください。';
  }
  if (message.startsWith('Invalid settings:')) return '変換設定を確認して、もう一度変換してください。';
  return '画像を変換できませんでした。画像と設定を確認して、もう一度お試しください。';
}

class ConversionSettings {
  const ConversionSettings({
    this.outputMode = OutputMode.preserve,
    this.gridWidth,
    this.colors,
    this.smoothing = 2,
    this.edgeProtection = 2,
    this.shapeProtection = 2,
    this.median = false,
  });
  final OutputMode outputMode;
  final int? gridWidth;
  final int? colors;
  final int smoothing;
  final int edgeProtection;
  final int shapeProtection;
  final bool median;
  Map<String, Object?> toJson() => {
    'output_mode': outputMode.name,
    'grid_width': gridWidth,
    'colors': colors,
    'smoothing': smoothing,
    'edge_protection': edgeProtection,
    'shape_protection': shapeProtection,
    'median': median,
  };
}

class ConversionResult {
  const ConversionResult(this.png, this.report);
  final Uint8List png;
  final Map<String, dynamic> report;
  int get width => report['output_width'] as int;
  int get height => report['output_height'] as int;
  int get gridWidth => (report['grid'] as Map)['width'] as int;
  int get gridHeight => (report['grid'] as Map)['height'] as int;
  int get cellPitch => report['cell_pitch'] as int;
  int get colorCount => (report['palette'] as List).length;
  double get milliseconds => (report['processing_ms'] as num).toDouble();
}

Future<ConversionResult> convertImage(
  Uint8List bytes,
  ConversionSettings settings,
) async {
  if (bytes.isEmpty || bytes.length > maxInputBytes) {
    throw const ImageConversionException('画像は1 byte〜64 MiBで指定してください。');
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
      throw const ImageConversionException('画像バッファを確保できません。');
    }
    input.asTypedList(bytes.length).setAll(0, bytes);
    options.asTypedList(json.length).setAll(0, json);
    result = native.convertNative(input, bytes.length, options, json.length);
    if (result == nullptr) {
      throw const ImageConversionException('変換結果を取得できません。');
    }
    final report = jsonDecode(
      utf8.decode(
        native.resultJson(result).asTypedList(native.resultJsonLength(result)),
      ),
    ) as Map<String, dynamic>;
    if (report.containsKey('error')) {
      throw ImageConversionException(
        _nativeErrorMessage(report['error'] as String),
      );
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
