import 'dart:convert';
import 'dart:ui' as ui;

import 'package:compter/main.dart';
import 'package:compter/src/appearance.dart';
import 'package:compter/src/compter.dart';
import 'package:compter/src/onboarding.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/frb_generated.dart';
import 'package:compter/src/ui/theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:path_provider/path_provider.dart';

/// The Play Store screenshots and feature graphic, drawn by the app itself on the demo
/// business: each is the real screen under a caption, rendered straight to a PNG of the size
/// Play wants. Run on an emulator with
/// `flutter drive --driver=test_driver/screenshots.dart --target=integration_test/screenshots_test.dart`;
/// the driver writes them into the fastlane metadata, which the `metadata` lane uploads.
void main() {
  final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => RustLib.init());
  // The store shows the app as people get it, without the debug ribbon
  WidgetsApp.debugAllowBannerOverride = false;

  final shots = <String, String>{};
  tearDownAll(() => binding.reportData = {'images': shots});

  final boundary = GlobalKey();

  /// The demo business as a phone screen under [caption], in a 9:16 frame
  Future<void> frame(
    WidgetTester tester,
    String caption,
    List<Override> overrides,
  ) async {
    final scratch = await getTemporaryDirectory();
    final compter = await CompterApp.demo(scratchDir: scratch.path);
    await tester.pumpWidget(
      _Frame(
        boundaryKey: boundary,
        caption: caption,
        child: ProviderScope(
          overrides: [compterProvider.overrideWithValue(compter), ...overrides],
          child: const CompterMobile(),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  /// Draw the frame as it stands to a PNG [width] pixels wide, under [name]
  Future<void> capture(
    WidgetTester tester,
    String name, {
    double width = 1080,
  }) async {
    await tester.pumpAndSettle();
    final render =
        boundary.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final image = await render.toImage(pixelRatio: width / render.size.width);
    final png = await image.toByteData(format: ui.ImageByteFormat.png);
    shots[name] = base64Encode(png!.buffer.asUint8List());
  }

  Future<void> tapText(WidgetTester tester, String text) async {
    final found = find.textContaining(text).first;
    await tester.ensureVisible(found);
    await tester.pumpAndSettle();
    await tester.tap(found);
    await tester.pumpAndSettle();
  }

  testWidgets('a project and its milestones', (tester) async {
    await frame(tester, 'Track every milestone\nand every hour', const []);
    await tapText(tester, 'Website Revamp');
    await capture(tester, 'phoneScreenshots/1_project');
  });

  testWidgets('issuing a GST invoice', (tester) async {
    await frame(
      tester,
      'GST invoices with the\ntax worked out for you',
      const [],
    );
    await tapText(tester, 'Website Revamp');
    await tapText(tester, 'Launch');
    await tapText(tester, 'Issue invoice');
    await tapText(tester, 'Preview');
    await capture(tester, 'phoneScreenshots/2_issue');
  });

  testWidgets('an invoice part paid', (tester) async {
    await frame(tester, 'Record part payments\nand the TDS withheld', const []);
    await tester.tap(find.byIcon(Icons.receipt_long_outlined));
    await tester.pumpAndSettle();
    await tapText(tester, 'Frontend');
    // A second part payment, with TDS withheld, as the caption promises
    await tapText(tester, 'Record payment');
    await tester.enterText(
      find.widgetWithText(TextField, 'Received (INR)'),
      '20,000',
    );
    await tester.enterText(find.widgetWithText(TextField, 'TDS'), '2,000');
    await tester.tap(find.text('Record'));
    await tester.pumpAndSettle();
    tester
        .state<ScaffoldMessengerState>(find.byType(ScaffoldMessenger).last)
        .removeCurrentSnackBar();
    await capture(tester, 'phoneScreenshots/3_invoice');
  });

  testWidgets('every project', (tester) async {
    await frame(tester, 'Every client and project\nat a glance', const []);
    await capture(tester, 'phoneScreenshots/4_projects');
  });

  testWidgets('the year in reports', (tester) async {
    await frame(
      tester,
      'Your year in numbers,\nfor 44ADA and GSTR-1',
      const [],
    );
    await tester.tap(find.byIcon(Icons.insights_outlined));
    await tester.pumpAndSettle();
    await capture(tester, 'phoneScreenshots/5_reports');
  });

  testWidgets('private by design', (tester) async {
    await frame(tester, 'No account, no ads:\nyour books stay yours', [
      showWelcomeProvider.overrideWithValue(true),
    ]);
    await tester.tap(find.text('Next'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Next'));
    await capture(tester, 'phoneScreenshots/6_private');
  });

  testWidgets('dark mode', (tester) async {
    await frame(tester, 'Light, dark and\ngruvbox themes', [
      appThemeProvider.overrideWith(() => AppThemeNotifier(AppTheme.dark)),
    ]);
    await tester.tap(find.byIcon(Icons.receipt_long_outlined));
    await capture(tester, 'phoneScreenshots/7_dark');
  });

  testWidgets('the feature graphic', (tester) async {
    await tester.pumpWidget(_FeatureGraphic(boundaryKey: boundary));
    await capture(tester, 'featureGraphic', width: 1024);
  });
}

/// The phone's screen size the app is laid out at, before it is scaled into the frame
const _screen = Size(412, 870);

final _brand = seededTheme(Brightness.light).colorScheme;

/// A caption over the app, drawn as a phone screen, in a 9:16 box
class _Frame extends StatelessWidget {
  const _Frame({
    required this.boundaryKey,
    required this.caption,
    required this.child,
  });

  final GlobalKey boundaryKey;
  final String caption;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Directionality(
      textDirection: TextDirection.ltr,
      child: Center(
        child: AspectRatio(
          aspectRatio: 9 / 16,
          child: RepaintBoundary(
            key: boundaryKey,
            child: ColoredBox(
              color: _brand.primary,
              child: LayoutBuilder(
                builder: (context, box) {
                  final unit = box.maxWidth / 100;
                  return Column(
                    children: [
                      SizedBox(
                        height: box.maxHeight * 0.22,
                        child: Center(
                          child: Text(
                            caption,
                            textAlign: TextAlign.center,
                            style: TextStyle(
                              color: _brand.onPrimary,
                              fontSize: 7 * unit,
                              fontWeight: FontWeight.w700,
                              height: 1.2,
                              decoration: TextDecoration.none,
                            ),
                          ),
                        ),
                      ),
                      Expanded(
                        child: Padding(
                          padding: EdgeInsets.fromLTRB(
                            12 * unit,
                            0,
                            12 * unit,
                            6 * unit,
                          ),
                          child: _Phone(child: child),
                        ),
                      ),
                    ],
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The app laid out at a phone's size, scaled down into a rounded bezel
class _Phone extends StatelessWidget {
  const _Phone({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final view = MediaQueryData.fromView(View.of(context));
    return FittedBox(
      child: Stack(
        children: [
          Padding(
            padding: const EdgeInsets.all(_rim),
            child: SizedBox.fromSize(
              size: _screen,
              // The app sees a phone of this size, with room for a status bar at the top
              child: MediaQuery(
                data: view.copyWith(
                  size: _screen,
                  padding: const EdgeInsets.only(top: 28),
                  viewPadding: const EdgeInsets.only(top: 28),
                  viewInsets: EdgeInsets.zero,
                ),
                child: child,
              ),
            ),
          ),
          // Painted over the app rather than clipping it: the frosted title and tab bars
          // blur past any clip
          const Positioned.fill(
            child: IgnorePointer(child: CustomPaint(painter: _Bezel())),
          ),
        ],
      ),
    );
  }
}

const _rim = 10.0;
const _radius = 48.0;

/// The phone's rim around the screen, and the caption band's colour outside it
class _Bezel extends CustomPainter {
  const _Bezel();

  @override
  void paint(Canvas canvas, Size size) {
    final box = Offset.zero & size;
    final outer = RRect.fromRectAndRadius(box, const Radius.circular(_radius));
    final inner = RRect.fromRectAndRadius(
      box.deflate(_rim),
      const Radius.circular(_radius - _rim),
    );
    canvas
      ..drawPath(
        Path.combine(
          PathOperation.difference,
          Path()..addRect(box),
          Path()..addRRect(outer),
        ),
        Paint()..color = _brand.primary,
      )
      ..drawDRRect(outer, inner, Paint()..color = const Color(0xFF1B1B1B));
  }

  @override
  bool shouldRepaint(_Bezel old) => false;
}

/// The 1024 × 500 banner over the listing: the name and what it is for
class _FeatureGraphic extends StatelessWidget {
  const _FeatureGraphic({required this.boundaryKey});

  final GlobalKey boundaryKey;

  @override
  Widget build(BuildContext context) {
    return Directionality(
      textDirection: TextDirection.ltr,
      child: Center(
        child: AspectRatio(
          aspectRatio: 1024 / 500,
          child: RepaintBoundary(
            key: boundaryKey,
            child: ColoredBox(
              color: _brand.primary,
              child: LayoutBuilder(
                builder: (context, box) {
                  final unit = box.maxWidth / 100;
                  final text = TextStyle(
                    color: _brand.onPrimary,
                    decoration: TextDecoration.none,
                  );
                  return Padding(
                    padding: EdgeInsets.symmetric(horizontal: 8 * unit),
                    child: Row(
                      children: [
                        CircleAvatar(
                          radius: 9 * unit,
                          backgroundColor: _brand.primaryContainer,
                          child: Icon(
                            Icons.receipt_long,
                            size: 10 * unit,
                            color: _brand.onPrimaryContainer,
                          ),
                        ),
                        SizedBox(width: 6 * unit),
                        Expanded(
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                'compter',
                                style: text.copyWith(
                                  fontSize: 9 * unit,
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              SizedBox(height: unit),
                              Text(
                                'GST invoices, payments and tax reports '
                                'for freelancers in India',
                                style: text.copyWith(
                                  fontSize: 3.6 * unit,
                                  height: 1.3,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );
  }
}
