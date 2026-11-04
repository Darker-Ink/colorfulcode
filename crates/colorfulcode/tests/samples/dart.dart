import 'dart:async';
import 'package:http/http.dart' as http;

/// A weather report for one city.
class Report {
  final String city;
  final double celsius;
  static const maxAge = Duration(minutes: 30);

  const Report(this.city, this.celsius);

  factory Report.fromJson(Map<String, dynamic> json) =>
      Report(json['city'] as String, (json['temp'] as num).toDouble());

  double get fahrenheit => celsius * 9 / 5 + 32;

  @override
  String toString() => 'Report($city, ${celsius.toStringAsFixed(1)}\u00B0C)';
}

enum Unit { metric, imperial }

mixin Logging {
  void log(String message) => print('[log] $message');
}

class WeatherService with Logging {
  final _cache = <String, Report>{};
  late final http.Client client;

  Future<Report?> fetch(String city, {Unit unit = Unit.metric}) async {
    if (_cache.containsKey(city)) return _cache[city];
    final raw = r'C:\weather\$city';
    log("fetching $city from $raw");
    final response = await client.get(Uri.parse('https://api.example.com/$city'));
    if (response.statusCode != 200) {
      throw StateError('failed: ${response.statusCode}');
    }
    return _cache[city] = Report(city, 21.5);
  }
}

void main() {
  final service = WeatherService();
  var count = 0x10;
  print('''Multi
line $count''');
}
