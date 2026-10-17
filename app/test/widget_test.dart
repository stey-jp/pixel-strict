import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelstrict/main.dart';
import 'package:pixelstrict/converter.dart';

void main() {
  final manifest = File('pubspec.yaml').readAsStringSync();
  final version = RegExp(
    r'^version:\s*(\S+)',
    multiLine: true,
  ).firstMatch(manifest)!.group(1)!;
  // Asset futures must not retain the previous widget test's fake-async zone.
  setUp(rootBundle.clear);
  testWidgets('Unsupported paths show Japanese messages', (tester) async {
    for (final (path, message) in [
      ('unsupported.txt', 'PNG・JPEG・WebP画像を選んでください。'),
    ]) {
      await tester.pumpWidget(PixelStrictApp(initialPath: path));
      await tester.pumpAndSettle();
      expect(find.text(message), findsOneWidget);
      expect(find.textContaining('FileSystemException'), findsNothing);
      expect(find.textContaining('Bad state:'), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    }
  });
  testWidgets(
    'Empty workbench disables conversion and save at mobile and desktop sizes',
    (tester) async {
      for (final size in [
        const Size(390, 844),
        const Size(1280, 800),
        const Size(1280, 600),
      ]) {
        await tester.binding.setSurfaceSize(size);
        await tester.pumpWidget(const PixelStrictApp());
        await tester.pumpAndSettle();
        expect(find.text('PixelStrict'), findsOneWidget);
        expect(
          tester.widget<Text>(find.byKey(const Key('appVersion'))).data,
          'v$version',
        );
        for (final text in [
          '曖昧なピクセルを、使える素材へ。',
          'Surface Strict + Edge Strict',
          '完全ローカル処理 · 画像は端末から送信されません',
          'ここから、ピクセルを整える。',
        ]) {
          expect(find.text(text), findsNothing);
        }
        final convertPosition = tester.getRect(
          find.byKey(const Key('convert')),
        );
        final savePosition = tester.getRect(find.byKey(const Key('save')));
        expect(convertPosition.top, greaterThanOrEqualTo(0));
        expect(savePosition.bottom, lessThanOrEqualTo(size.height));
        final shape = find.descendant(
          of: find.byKey(const Key('shapeProtection')),
          matching: find.byType(SegmentedButton<int>),
        );
        expect(tester.widget<SegmentedButton<int>>(shape).selected, {2});
        await tester.ensureVisible(shape);
        await tester.tap(find.descendant(of: shape, matching: find.text('強')));
        await tester.pumpAndSettle();
        expect(tester.widget<SegmentedButton<int>>(shape).selected, {3});
        final output = find.byKey(const Key('outputMode'));
        expect(
          tester
              .widget<DropdownButtonFormField<OutputMode>>(output)
              .initialValue,
          OutputMode.preserve,
        );
        expect(find.byKey(const Key('outputHint')), findsOneWidget);
        await tester.ensureVisible(output);
        await tester.tap(output);
        await tester.pumpAndSettle();
        await tester.tap(find.text('Logical pixels').last);
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<DropdownButtonFormField<OutputMode>>(output)
              .initialValue,
          OutputMode.logical,
        );
        expect(find.byKey(const Key('outputHint')), findsNothing);
        expect(
          tester
              .widget<FilledButton>(find.byKey(const Key('convert')))
              .onPressed,
          isNull,
        );
        expect(
          tester.getRect(find.byKey(const Key('convert'))),
          convertPosition,
        );
        expect(tester.getRect(find.byKey(const Key('save'))), savePosition);
        expect(
          tester
              .widget<OutlinedButton>(find.byKey(const Key('save')))
              .onPressed,
          isNull,
        );
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      }
      await tester.binding.setSurfaceSize(null);
    },
  );
}
