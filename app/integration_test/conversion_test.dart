import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:pixelstrict/main.dart';
import 'package:pixelstrict/converter.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets(
    'Windows converts over FFI, synchronizes previews, and invalidates stale output',
    (tester) async {
      final path = const String.fromEnvironment('SAMPLE_PATH');
      expect(
        path,
        isNotEmpty,
        reason: 'Pass --dart-define=SAMPLE_PATH=/absolute/house-pseudo.png',
      );
      await tester.pumpWidget(PixelStrictApp(initialPath: path));
      for (var i = 0; i < 100; i++) {
        await tester.pump(const Duration(milliseconds: 100));
        if (tester
                .widget<FilledButton>(find.byKey(const Key('convert')))
                .onPressed !=
            null) {
          break;
        }
      }
      expect(find.text('house-pseudo.png'), findsOneWidget);
      final viewers = find.byType(InteractiveViewer);
      Future<void> zoom(int index) async {
        await tester.ensureVisible(viewers.at(index));
        await tester.sendEventToBinding(
          PointerScrollEvent(
            position: tester.getCenter(viewers.at(index)),
            scrollDelta: const Offset(0, -160),
          ),
        );
        await tester.pumpAndSettle();
      }

      await zoom(0);
      final viewController = tester
          .widget<InteractiveViewer>(viewers.first)
          .transformationController!;
      final beforeConversion = viewController.value.clone();
      expect(beforeConversion.getMaxScaleOnAxis(), greaterThan(1));
      await tester.ensureVisible(find.byKey(const Key('convert')));
      await tester.tap(find.byKey(const Key('convert')));
      for (var i = 0; i < 200; i++) {
        await tester.pump(const Duration(milliseconds: 100));
        if (find.byKey(const Key('resultInfo')).evaluate().isNotEmpty) break;
      }
      expect(find.byKey(const Key('resultInfo')), findsOneWidget);
      expect(
        tester.widget<Text>(find.byKey(const Key('resultInfo'))).data,
        contains('32 × 32'),
      );
      expect(
        tester.widget<OutlinedButton>(find.byKey(const Key('save'))).onPressed,
        isNotNull,
      );
      await tester.pumpAndSettle();
      final previews = find.byWidgetPredicate(
        (widget) => widget is Image && widget.image is MemoryImage,
      );
      expect(previews, findsNWidgets(2));
      expect(tester.getSize(previews.at(0)).width, greaterThan(200));
      expect(
        tester.getSize(previews.at(1)).width,
        tester.getSize(previews.at(0)).width,
      );
      void expectLinkedPreviews() {
        final transforms = [
          for (var i = 0; i < 2; i++)
            tester
                .renderObject<RenderBox>(previews.at(i))
                .getTransformTo(tester.renderObject<RenderBox>(viewers.at(i))),
        ];
        for (var i = 0; i < 16; i++) {
          expect(
            transforms[0].storage[i],
            closeTo(transforms[1].storage[i], 1e-8),
          );
        }
        expect(transforms[0].getMaxScaleOnAxis(), greaterThan(1));
      }

      // A freshly converted image inherits the zoom chosen on the source.
      expect(
        viewController.value.storage,
        orderedEquals(beforeConversion.storage),
      );
      expectLinkedPreviews();
      for (final index in [1, 0]) {
        final oldScale = viewController.value.getMaxScaleOnAxis();
        await zoom(index);
        expect(viewController.value.getMaxScaleOnAxis(), greaterThan(oldScale));
        expectLinkedPreviews();
        final oldTranslation = viewController.value.getTranslation();
        await tester.drag(viewers.at(index), const Offset(35, 25));
        await tester.pumpAndSettle();
        expect(
          viewController.value.getTranslation().x,
          greaterThan(oldTranslation.x),
        );
        expect(
          viewController.value.getTranslation().y,
          greaterThan(oldTranslation.y),
        );
        expectLinkedPreviews();
      }
      final source = await File(path).readAsBytes();
      final a = await convertImage(
        source,
        const ConversionSettings(gridWidth: 32),
      );
      final b = await convertImage(
        source,
        const ConversionSettings(gridWidth: 32),
      );
      expect(a.png, orderedEquals(b.png));
      final temp = await Directory.systemTemp.createTemp('pixelstrict-test-');
      final saved = File('${temp.path}/strict.png');
      try {
        await saved.writeAsBytes(a.png);
        expect(await saved.readAsBytes(), orderedEquals(a.png));
      } finally {
        await temp.delete(recursive: true);
      }
      await tester.ensureVisible(find.text('Manual'));
      await tester.tap(find.text('Manual'));
      await tester.pump();
      expect(find.byKey(const Key('gridWidth')), findsOneWidget);
      expect(find.byKey(const Key('resultInfo')), findsNothing);
      expect(
        tester.widget<OutlinedButton>(find.byKey(const Key('save'))).onPressed,
        isNull,
      );
      expect(tester.takeException(), isNull);
    },
  );
}
