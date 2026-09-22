import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelstrict/main.dart';

void main() {
  testWidgets(
    'Empty workbench disables conversion and save at mobile and desktop sizes',
    (tester) async {
      for (final size in [const Size(390, 844), const Size(1280, 800)]) {
        await tester.binding.setSurfaceSize(size);
        await tester.pumpWidget(const PixelStrictApp());
        await tester.pump();
        expect(find.text('PixelStrict'), findsOneWidget);
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
      }
      await tester.binding.setSurfaceSize(null);
    },
  );
}
