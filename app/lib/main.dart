import 'dart:io';
import 'dart:ui' as ui;

import 'package:desktop_drop/desktop_drop.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'converter.dart';

const _background = Color(0xff111515);
const _panel = Color(0xff1b2020);
const _line = Color(0xff343d3b);
const _muted = Color(0xffacb8b1);
const _accent = Color(0xffd5fa7a);

void main(List<String> args) {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(
    PixelStrictApp(
      initialPath: args.where((a) => !a.startsWith('-')).firstOrNull,
    ),
  );
}

class PixelStrictApp extends StatelessWidget {
  const PixelStrictApp({super.key, this.initialPath});
  final String? initialPath;
  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'PixelStrict',
    debugShowCheckedModeBanner: false,
    theme: ThemeData(
      brightness: Brightness.dark,
      scaffoldBackgroundColor: _background,
      colorScheme: const ColorScheme.dark(
        primary: _accent,
        onPrimary: _background,
        surface: _panel,
        outline: _line,
      ),
      fontFamily: Platform.isWindows ? 'Yu Gothic UI' : null,
      dividerColor: _line,
      inputDecorationTheme: const InputDecorationTheme(
        border: OutlineInputBorder(),
        isDense: true,
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size(48, 48),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        ),
      ),
    ),
    home: Workbench(initialPath: initialPath),
  );
}

class Workbench extends StatefulWidget {
  const Workbench({super.key, this.initialPath});
  final String? initialPath;
  @override
  State<Workbench> createState() => _WorkbenchState();
}

class _WorkbenchState extends State<Workbench> {
  late final _version = rootBundle
      .loadString('pubspec.yaml')
      .then(
        (source) => RegExp(
          r'^version:\s*(\S+)',
          multiLine: true,
        ).firstMatch(source)!.group(1)!,
      );
  final _gridController = TextEditingController(text: '64');
  final _previewController = TransformationController();
  Uint8List? _source;
  String _name = '';
  int _width = 0, _height = 0;
  ConversionResult? _result;
  bool _busy = false, _dragging = false, _manual = false, _median = false;
  int _colors = 0, _smoothing = 2, _edge = 2, _shape = 2;
  OutputMode _outputMode = OutputMode.preserve;
  String? _error;
  String _activity = '';

  @override
  void initState() {
    super.initState();
    if (widget.initialPath != null) {
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => _loadPath(widget.initialPath!),
      );
    }
  }

  @override
  void dispose() {
    _gridController.dispose();
    _previewController.dispose();
    super.dispose();
  }

  void _change(VoidCallback update) => setState(() {
    update();
    _result = null;
    _error = null;
  });
  Future<void> _guard(String activity, Future<void> Function() action) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
      _activity = activity;
    });
    try {
      await action();
    } catch (error) {
      if (mounted) {
        setState(
          () => _error = error.toString().replaceFirst('FormatException: ', ''),
        );
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _accept(Uint8List bytes, String name) async {
    if (bytes.isEmpty || bytes.length > maxInputBytes) {
      throw const FormatException('64 MiB以下の画像を選択してください。');
    }
    final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
    ui.ImageDescriptor? descriptor;
    try {
      descriptor = await ui.ImageDescriptor.encoded(buffer);
      final width = descriptor.width, height = descriptor.height;
      if (width > 8192 || height > 8192 || width * height > 16777216) {
        throw const FormatException('画像上限は各辺8192px・合計16メガピクセルです。');
      }
      if (!mounted) return;
      setState(() {
        _previewController.value = Matrix4.identity();
        _source = bytes;
        _name = name;
        _width = width;
        _height = height;
        _result = null;
      });
    } finally {
      descriptor?.dispose();
      buffer.dispose();
    }
  }

  Future<void> _loadPath(String path) => _guard('画像を読み込み中', () async {
    final file = File(path);
    if (await file.length() > maxInputBytes) {
      throw const FormatException('64 MiB以下の画像を選択してください。');
    }
    await _accept(await file.readAsBytes(), path.split(RegExp(r'[/\\]')).last);
  });

  Future<void> _pick() => _guard('画像を読み込み中', () async {
    final file = await FilePicker.pickFile(
      type: FileType.custom,
      allowedExtensions: ['png', 'jpg', 'jpeg', 'webp'],
    );
    if (file == null) return;
    final length = await file.length();
    if (length != null && length > maxInputBytes) {
      throw const FormatException('64 MiB以下の画像を選択してください。');
    }
    await _accept(await file.readAsBytes(), file.name);
  });

  Future<void> _convert() => _guard('画像を変換中', () async {
    final width = _manual ? int.tryParse(_gridController.text) : null;
    if (_manual && (width == null || width < 1 || width > _width)) {
      throw FormatException('Grid幅は1〜$_widthの整数で指定してください。');
    }
    final result = await convertImage(
      _source!,
      ConversionSettings(
        outputMode: _outputMode,
        gridWidth: width,
        colors: _colors == 0 ? null : _colors,
        smoothing: _smoothing,
        edgeProtection: _edge,
        shapeProtection: _shape,
        median: _median,
      ),
    );
    if (mounted) setState(() => _result = result);
  });

  Future<void> _save() => _guard('PNGを保存中', () async {
    final result = _result!;
    final base = _name.replaceFirst(RegExp(r'\.[^.]+$'), '');
    final uri = await FilePicker.saveFile(
      dialogTitle: 'Strict PNGを保存',
      fileName: '$base-strict.png',
      type: FileType.custom,
      allowedExtensions: ['png'],
      bytes: result.png,
    );
    if (uri != null && mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('Strict PNGを保存しました。')));
    }
  });

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 20),
            child: LayoutBuilder(
              builder: (context, constraints) => Row(
                children: [
                  const _PixelMark(),
                  const SizedBox(width: 12),
                  const Expanded(
                    child: Text(
                      'PixelStrict',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 23,
                        fontWeight: FontWeight.w700,
                        letterSpacing: -0.7,
                      ),
                    ),
                  ),
                  FutureBuilder<String>(
                    future: _version,
                    builder: (context, snapshot) => Text(
                      snapshot.hasData ? 'v${snapshot.data}' : '',
                      key: const Key('appVersion'),
                      style: const TextStyle(color: _muted, fontSize: 11),
                    ),
                  ),
                ],
              ),
            ),
          ),
          const Divider(height: 1),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constraints) {
                if (constraints.maxWidth >= 1000) {
                  return Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      SizedBox(
                        width: 292,
                        child: Column(
                          children: [
                            Expanded(
                              child: SingleChildScrollView(
                                key: const Key('settingsScroll'),
                                padding: const EdgeInsets.all(24),
                                child: _settings(),
                              ),
                            ),
                            _actions(),
                          ],
                        ),
                      ),
                      const VerticalDivider(width: 1),
                      Expanded(
                        child: Padding(
                          padding: const EdgeInsets.all(28),
                          child: _workspace(true),
                        ),
                      ),
                    ],
                  );
                }
                return SingleChildScrollView(
                  key: const Key('workbenchScroll'),
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    children: [
                      _workspace(false),
                      const SizedBox(height: 28),
                      _settings(),
                    ],
                  ),
                );
              },
            ),
          ),
          LayoutBuilder(
            builder: (context, constraints) => constraints.maxWidth < 1000
                ? _actions(horizontal: true)
                : const SizedBox.shrink(),
          ),
        ],
      ),
    ),
  );

  Widget _settings() => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      OutlinedButton.icon(
        onPressed: _busy ? null : _pick,
        icon: const Icon(Icons.add_photo_alternate_outlined, size: 19),
        label: const Text('画像を選択'),
        style: OutlinedButton.styleFrom(minimumSize: const Size(48, 48)),
      ),
      const SizedBox(height: 28),
      _label('Output', 'PNGの出力サイズ'),
      DropdownButtonFormField<OutputMode>(
        key: const Key('outputMode'),
        isExpanded: true,
        initialValue: _outputMode,
        items: const [
          DropdownMenuItem(
            value: OutputMode.preserve,
            child: Text('Preserve size'),
          ),
          DropdownMenuItem(
            value: OutputMode.logical,
            child: Text('Logical pixels'),
          ),
        ],
        onChanged: _busy ? null : (v) => _change(() => _outputMode = v!),
      ),
      const SizedBox(height: 24),
      _label('Grid', '正規グリッドの解像度'),
      SegmentedButton<bool>(
        segments: const [
          ButtonSegment(value: false, label: Text('Auto')),
          ButtonSegment(value: true, label: Text('Manual')),
        ],
        selected: {_manual},
        onSelectionChanged: _busy
            ? null
            : (v) => _change(() => _manual = v.first),
      ),
      if (_manual)
        Padding(
          padding: const EdgeInsets.only(top: 12),
          child: TextField(
            key: const Key('gridWidth'),
            controller: _gridController,
            enabled: !_busy,
            keyboardType: TextInputType.number,
            inputFormatters: [FilteringTextInputFormatter.digitsOnly],
            decoration: InputDecoration(
              labelText: 'Grid width（セル数）',
              helperText: _gridHint,
            ),
            onChanged: (_) => _change(() {}),
          ),
        ),
      const SizedBox(height: 24),
      _label('Colors', '透明色を含む最大色数'),
      SegmentedButton<int>(
        showSelectedIcon: false,
        segments: [
          for (final n in [0, 16, 24, 32])
            ButtonSegment(value: n, label: Text(n == 0 ? 'Auto' : '$n')),
        ],
        selected: {_colors},
        onSelectionChanged: _busy
            ? null
            : (v) => _change(() => _colors = v.first),
      ),
      const SizedBox(height: 24),
      _label('Surface smoothing', '単色面のざらつきを整理'),
      _strength(_smoothing, (n) => _smoothing = n),
      const SizedBox(height: 24),
      _label('Edge protection', '輪郭・斜線・細線を保持'),
      _strength(_edge, (n) => _edge = n),
      const SizedBox(height: 24),
      _label('Shape protection', '外形・細線・小さな形を保持'),
      KeyedSubtree(
        key: const Key('shapeProtection'),
        child: _strength(_shape, (n) => _shape = n),
      ),
      const SizedBox(height: 20),
      SwitchListTile(
        contentPadding: EdgeInsets.zero,
        title: const Text('Median', style: TextStyle(fontSize: 14)),
        subtitle: Text(
          _median ? '1px · 軽い前処理' : 'Off · 細部を優先',
          style: const TextStyle(color: _muted, fontSize: 12),
        ),
        value: _median,
        onChanged: _busy ? null : (v) => _change(() => _median = v),
      ),
    ],
  );

  Widget _actions({bool horizontal = false}) {
    final convert = FilledButton.icon(
      key: const Key('convert'),
      onPressed: _source == null || _busy ? null : _convert,
      icon: const Icon(Icons.auto_fix_high, size: 19),
      label: Text(_busy ? '処理中…' : 'Strictへ変換'),
    );
    final save = OutlinedButton.icon(
      key: const Key('save'),
      onPressed: _result == null || _busy ? null : _save,
      icon: const Icon(Icons.save_alt, size: 18),
      label: const Text('PNGを保存'),
      style: OutlinedButton.styleFrom(minimumSize: const Size(48, 48)),
    );
    return Padding(
      key: const Key('fixedActions'),
      padding: EdgeInsets.all(horizontal ? 16 : 24),
      child: horizontal
          ? Row(
              children: [
                Expanded(child: convert),
                const SizedBox(width: 10),
                Expanded(child: save),
              ],
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [convert, const SizedBox(height: 10), save],
            ),
    );
  }

  String get _gridHint {
    if (_outputMode == OutputMode.logical) return '高さは元画像の比率から決定';
    final columns = int.tryParse(_gridController.text);
    if (columns != null &&
        columns > 0 &&
        columns <= _width &&
        _width % columns == 0) {
      final pitch = _width ~/ columns;
      if (_height % pitch == 0) return '$columns cells / ${pitch}px';
    }
    return '元画像の両辺を割り切る正方形セルのみ';
  }

  Widget _label(String title, String subtitle) => Padding(
    padding: const EdgeInsets.only(bottom: 10),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 14),
        ),
        const SizedBox(height: 4),
        Text(subtitle, style: const TextStyle(color: _muted, fontSize: 11)),
      ],
    ),
  );
  Widget _strength(int value, void Function(int) update) =>
      SegmentedButton<int>(
        segments: const [
          ButtonSegment(value: 1, label: Text('弱')),
          ButtonSegment(value: 2, label: Text('中')),
          ButtonSegment(value: 3, label: Text('強')),
        ],
        selected: {value},
        onSelectionChanged: _busy
            ? null
            : (v) => _change(() => update(v.first)),
      );

  Widget _workspace(bool wide) {
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(bottom: 16),
            child: Semantics(
              liveRegion: true,
              child: Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          ),
        if (_busy)
          Padding(
            padding: const EdgeInsets.only(bottom: 16),
            child: Semantics(
              liveRegion: true,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    _activity,
                    style: const TextStyle(color: _muted, fontSize: 12),
                  ),
                  const SizedBox(height: 8),
                  const LinearProgressIndicator(minHeight: 2),
                ],
              ),
            ),
          ),
        if (wide)
          Expanded(child: _previews(true))
        else
          SizedBox(
            height: _source == null ? 320 : 600,
            child: _previews(false),
          ),
        if (_result != null)
          Padding(
            padding: const EdgeInsets.only(top: 14),
            child: Text(
              '${_result!.width} × ${_result!.height} px  ·  ${_result!.colorCount} colors\nGrid ${_result!.gridWidth} × ${_result!.gridHeight} cells / ${_result!.cellPitch}px\n${_result!.milliseconds.toStringAsFixed(0)} ms  ·  ${(_result!.report['candidates'] as List).length}候補を評価',
              key: const Key('resultInfo'),
              style: const TextStyle(color: _accent, fontSize: 13),
            ),
          ),
      ],
    );
    if (!(Platform.isWindows || Platform.isMacOS)) return content;
    return DropTarget(
      enable: !_busy,
      onDragEntered: (_) => setState(() => _dragging = true),
      onDragExited: (_) => setState(() => _dragging = false),
      onDragDone: (detail) {
        setState(() => _dragging = false);
        if (detail.files.isNotEmpty) _loadPath(detail.files.first.path);
      },
      child: DecoratedBox(
        decoration: BoxDecoration(
          border: _dragging ? Border.all(color: _accent, width: 2) : null,
        ),
        child: content,
      ),
    );
  }

  Widget _previews(bool wide) {
    if (_source == null) {
      return Container(
        width: double.infinity,
        decoration: BoxDecoration(
          color: _panel,
          border: Border.all(color: _line),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(
                  Icons.add_photo_alternate_outlined,
                  size: 42,
                  color: _accent,
                ),
                const SizedBox(height: 20),
                Text(
                  Platform.isWindows || Platform.isMacOS
                      ? '画像をドロップ、またはファイルを選択'
                      : 'ファイルから画像を選択',
                  style: const TextStyle(color: _muted, fontSize: 12),
                ),
                const SizedBox(height: 24),
                FilledButton(
                  onPressed: _busy ? null : _pick,
                  child: const Text('画像を選択'),
                ),
                const SizedBox(height: 16),
                const Text(
                  'PNG / JPEG / WebP  ·  最大16 MP',
                  style: TextStyle(color: _muted, fontSize: 11),
                ),
              ],
            ),
          ),
        ),
      );
    }
    final before = _Preview(
      controller: _previewController,
      title: 'BEFORE',
      subtitle: '$_width × $_height px',
      bytes: _source,
      name: _name,
    );
    final after = _Preview(
      controller: _previewController,
      title: 'AFTER',
      subtitle: _result == null
          ? '変換待ち'
          : '${_result!.width} × ${_result!.height} px',
      bytes: _result?.png,
      name: '${_name.replaceFirst(RegExp(r'\.[^.]+$'), '')}-strict.png',
      accent: true,
    );
    return wide
        ? Row(
            children: [
              Expanded(child: before),
              const SizedBox(width: 16),
              Expanded(child: after),
            ],
          )
        : Column(
            children: [
              Expanded(child: before),
              const SizedBox(height: 16),
              Expanded(child: after),
            ],
          );
  }
}

class _Preview extends StatelessWidget {
  const _Preview({
    required this.controller,
    required this.title,
    required this.subtitle,
    required this.bytes,
    required this.name,
    this.accent = false,
  });
  final String title, subtitle, name;
  final TransformationController controller;
  final Uint8List? bytes;
  final bool accent;
  @override
  Widget build(BuildContext context) => Container(
    clipBehavior: Clip.antiAlias,
    decoration: BoxDecoration(
      border: Border.all(color: _line),
      borderRadius: BorderRadius.circular(10),
    ),
    child: Column(
      children: [
        Container(
          color: _panel,
          padding: const EdgeInsets.all(14),
          child: Row(
            children: [
              Text(
                title,
                style: TextStyle(
                  fontSize: 11,
                  letterSpacing: 1.5,
                  fontWeight: FontWeight.bold,
                  color: accent ? _accent : _muted,
                ),
              ),
              const Spacer(),
              Text(
                subtitle,
                style: const TextStyle(fontSize: 11, color: _muted),
              ),
            ],
          ),
        ),
        Expanded(
          child: CustomPaint(
            painter: _Checkerboard(),
            child: SizedBox.expand(
              child: bytes == null
                  ? const Center(
                      child: Text(
                        '変換すると、ここに表示されます',
                        textAlign: TextAlign.center,
                        style: TextStyle(color: _muted, fontSize: 12),
                      ),
                    )
                  : InteractiveViewer(
                      key: ValueKey(bytes),
                      transformationController: controller,
                      minScale: 1,
                      maxScale: 32,
                      child: Center(
                        child: Image.memory(
                          bytes!,
                          width: double.infinity,
                          height: double.infinity,
                          fit: BoxFit.contain,
                          filterQuality: FilterQuality.none,
                          isAntiAlias: false,
                          semanticLabel: '$title $name',
                          errorBuilder: (_, _, _) =>
                              const Text('プレビューを表示できません'),
                        ),
                      ),
                    ),
            ),
          ),
        ),
        Container(
          width: double.infinity,
          color: _panel,
          padding: const EdgeInsets.all(12),
          child: Text(
            name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(color: _muted, fontSize: 10),
          ),
        ),
      ],
    ),
  );
}

class _Checkerboard extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRect(
      Offset.zero & size,
      Paint()..color = const Color(0xff171c1b),
    );
    final paint = Paint()..color = const Color(0xff202625);
    for (var y = 0; y < size.height; y += 16) {
      for (var x = 0; x < size.width; x += 16) {
        if ((x ~/ 16 + y ~/ 16).isEven) {
          canvas.drawRect(
            Rect.fromLTWH(x.toDouble(), y.toDouble(), 16, 16),
            paint,
          );
        }
      }
    }
  }

  @override
  bool shouldRepaint(_Checkerboard oldDelegate) => false;
}

class _PixelMark extends StatelessWidget {
  const _PixelMark();
  @override
  Widget build(BuildContext context) => Image.asset(
    'assets/branding/pixelstrict-mark.png',
    width: 26,
    height: 26,
    filterQuality: FilterQuality.none,
    excludeFromSemantics: true,
  );
}
