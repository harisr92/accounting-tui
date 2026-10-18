import 'package:compter/src/ui/search.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('every word must appear in some field, ignoring case', () {
    final query = SearchQuery('acme LAUNCH');
    expect(query.matches(['Acme Pvt Ltd', 'Launch']), isTrue);
    expect(query.matches(['Acme Pvt Ltd', 'Design']), isFalse);
  });

  test('a word may match inside any one field', () {
    final query = SearchQuery('NDC/26');
    expect(query.matches(['Acme', 'NDC/26-27/0003']), isTrue);
  });

  test('blank or spaces match everything', () {
    expect(SearchQuery('   ').isEmpty, isTrue);
    expect(SearchQuery('').apply([1, 2, 3], (_) => ['x']), [1, 2, 3]);
  });

  test('extra whitespace is ignored and the text is kept trimmed', () {
    final query = SearchQuery('  globex   app ');
    expect(query.text, 'globex   app');
    expect(query.matches(['Mobile App', 'Globex Labs']), isTrue);
  });

  test('apply keeps the matching rows in order', () {
    final rows = ['Laptop', 'Fibre broadband', 'Laptop stand'];
    expect(SearchQuery('laptop').apply(rows, (row) => [row]), [
      'Laptop',
      'Laptop stand',
    ]);
  });
}
