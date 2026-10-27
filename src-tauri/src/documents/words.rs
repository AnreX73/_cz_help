//! Сумма прописью для товарного чека.

const UNITS_M: [&str; 10] = [
    "", "один", "два", "три", "четыре", "пять", "шесть", "семь", "восемь", "девять",
];
const UNITS_F: [&str; 10] = [
    "", "одна", "две", "три", "четыре", "пять", "шесть", "семь", "восемь", "девять",
];
const TEENS: [&str; 10] = [
    "десять",
    "одиннадцать",
    "двенадцать",
    "тринадцать",
    "четырнадцать",
    "пятнадцать",
    "шестнадцать",
    "семнадцать",
    "восемнадцать",
    "девятнадцать",
];
const TENS: [&str; 10] = [
    "", "", "двадцать", "тридцать", "сорок", "пятьдесят", "шестьдесят", "семьдесят",
    "восемьдесят", "девяносто",
];
const HUNDREDS: [&str; 10] = [
    "", "сто", "двести", "триста", "четыреста", "пятьсот", "шестьсот", "семьсот", "восемьсот",
    "девятьсот",
];

fn plural(n: i64, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
    let n = n.abs() % 100;
    if (11..=14).contains(&n) {
        return many;
    }
    match n % 10 {
        1 => one,
        2..=4 => few,
        _ => many,
    }
}

fn triad_words(mut n: i64, feminine: bool) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let hundreds = (n / 100) as usize;
    if hundreds > 0 {
        parts.push(HUNDREDS[hundreds]);
        n %= 100;
    }
    if (10..20).contains(&n) {
        parts.push(TEENS[(n - 10) as usize]);
        return parts.join(" ");
    }
    let tens = (n / 10) as usize;
    if tens > 0 {
        parts.push(TENS[tens]);
        n %= 10;
    }
    let units = n as usize;
    if units > 0 {
        parts.push(if feminine { UNITS_F[units] } else { UNITS_M[units] });
    }
    parts.join(" ")
}

fn number_words(n: i64, feminine: bool) -> String {
    if n == 0 {
        return "ноль".into();
    }
    let mut parts: Vec<String> = Vec::new();
    let millions = n / 1_000_000;
    let thousands = (n / 1_000) % 1_000;
    let rest = n % 1_000;

    if millions > 0 {
        parts.push(format!(
            "{} {}",
            triad_words(millions, false),
            plural(millions, "миллион", "миллиона", "миллионов")
        ));
    }
    if thousands > 0 {
        parts.push(format!(
            "{} {}",
            triad_words(thousands, true),
            plural(thousands, "тысяча", "тысячи", "тысяч")
        ));
    }
    if rest > 0 {
        parts.push(triad_words(rest, feminine));
    }
    parts.join(" ")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// «Шестьсот десять рублей 00 копеек»
pub fn amount_in_words(kop: i64) -> String {
    let kop = kop.max(0);
    let rub = kop / 100;
    let cents = kop % 100;
    let ruble_form = plural(rub, "рубль", "рубля", "рублей");
    let cent_form = plural(cents, "копейка", "копейки", "копеек");
    capitalize(&format!(
        "{} {} {:02} {}",
        number_words(rub, false),
        ruble_form,
        cents,
        cent_form
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_amount() {
        assert_eq!(amount_in_words(610_00), "Шестьсот десять рублей 00 копеек");
    }

    #[test]
    fn singular_ruble_and_kopeck() {
        assert_eq!(amount_in_words(1_01), "Один рубль 01 копейка");
    }

    #[test]
    fn few_forms() {
        assert_eq!(amount_in_words(2_02), "Два рубля 02 копейки");
        assert_eq!(amount_in_words(4_44), "Четыре рубля 44 копейки");
    }

    #[test]
    fn thousands_feminine() {
        assert_eq!(amount_in_words(1_000_00), "Одна тысяча рублей 00 копеек");
        assert_eq!(
            amount_in_words(2_000_00),
            "Две тысячи рублей 00 копеек"
        );
        assert_eq!(
            amount_in_words(5_000_00),
            "Пять тысяч рублей 00 копеек"
        );
    }

    #[test]
    fn millions_and_teens() {
        assert_eq!(
            amount_in_words(2_400_000_00),
            "Два миллиона четыреста тысяч рублей 00 копеек"
        );
        assert_eq!(
            amount_in_words(11_00),
            "Одиннадцать рублей 00 копеек"
        );
    }

    #[test]
    fn eleven_to_fourteen_use_many() {
        assert_eq!(amount_in_words(11_11), "Одиннадцать рублей 11 копеек");
        assert_eq!(amount_in_words(12_14), "Двенадцать рублей 14 копеек");
    }

    #[test]
    fn zero() {
        assert_eq!(amount_in_words(0), "Ноль рублей 00 копеек");
    }
}
