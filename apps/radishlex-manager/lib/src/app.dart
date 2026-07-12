import 'package:flutter/material.dart';

import 'bridge/fixture_manager_bridge.dart';
import 'bridge/manager_bridge.dart';
import 'screens/manager_home_screen.dart';

class RadishLexManagerApp extends StatelessWidget {
  const RadishLexManagerApp({super.key, this.bridge});

  final ManagerBridge? bridge;

  @override
  Widget build(BuildContext context) {
    const seedColor = Color(0xFF256F6A);
    final colorScheme = ColorScheme.fromSeed(
      seedColor: seedColor,
      brightness: Brightness.light,
    );

    return MaterialApp(
      title: 'RadishLex Manager',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        colorScheme: colorScheme,
        scaffoldBackgroundColor: const Color(0xFFF6F7F8),
        cardTheme: CardThemeData(
          color: Colors.white,
          elevation: 0,
          margin: EdgeInsets.zero,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(8),
            side: BorderSide(color: colorScheme.outlineVariant),
          ),
        ),
        inputDecorationTheme: InputDecorationTheme(
          border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
          isDense: true,
        ),
      ),
      home: ManagerHomeScreen(bridge: bridge ?? FixtureManagerBridge()),
    );
  }
}
