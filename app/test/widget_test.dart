import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelstrict/main.dart';
import 'package:pixelstrict/converter.dart';

void main() {
  testWidgets(
    'Empty workbench disables conversion and save at mobile and desktop sizes',
    (tester) async {
      for (final size in [const Size(390, 844), const Size(1280, 800)]) {
        await tester.binding.setSurfaceSize(size);
        await tester.pumpWidget(const PixelStrictApp());
        await tester.pump();
        expect(find.text('PixelStrict'), findsOneWidget);
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
        await tester.ensureVisible(output);
        await tester.tap(output);
        await tester.pumpAndSettle();
        await tester.tap(find.text('Logical pixels').last);
        await tester.pumpAndSettle();
        expect(find.text('1セル = 1ピクセルのPNG\n半透明・補間・ディザリングなし'), findsOneWidget);
        expect(
          tester
              .widget<DropdownButtonFormField<OutputMode>>(output)
              .initialValue,
          OutputMode.logical,
        );
        expect(
          tester
              .widget<FilledButton>(find.byKey(const Key('convert')))
              .onPressed,
          isNull,
        );
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
