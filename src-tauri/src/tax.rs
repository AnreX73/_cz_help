use crate::models::MonthReport;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct EntryLite {
    pub date: String,
    pub amount_kop: i64,
    pub payer_type: String,
}

#[derive(Debug, Clone, Copy)]
pub struct TaxRules {
    /// Ставка в сотых долях процента: 400 = 4 %.
    pub rate_individual: i64,
    /// Ставка в сотых долях процента: 600 = 6 %.
    pub rate_organization: i64,
    pub deduction_kop: i64,
}

impl TaxRules {
    pub fn from_settings(s: &crate::models::Settings) -> Self {
        Self {
            rate_individual: s.rate_individual,
            rate_organization: s.rate_organization,
            deduction_kop: s.deduction_kop,
        }
    }
}

/// Деление с округлением половины вверх, только для неотрицательных значений.
fn div_round(n: i128, d: i128) -> i64 {
    if d == 0 {
        return 0;
    }
    ((n + d / 2) / d) as i64
}

fn parse_ym(date: &str) -> Option<(i64, i64)> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<i64>().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    Some((year, month))
}

/// Налог по одной операции: возвращает (налог, сколько вычета потрачено).
fn tax_for_entry(amount_kop: i64, organization: bool, rules: &TaxRules, deduction_left: i64) -> (i64, i64) {
    let amount = amount_kop.max(0);
    let rate = if organization {
        rules.rate_organization
    } else {
        rules.rate_individual
    };
    let gross = div_round(amount as i128 * rate as i128, 10_000);
    if !organization {
        return (gross, 0);
    }
    let extra = (rules.rate_organization - rules.rate_individual).max(0);
    let cap = div_round(amount as i128 * extra as i128, 10_000);
    let used = cap.min(deduction_left).max(0);
    ((gross - used).max(0), used)
}

/// Считает налог по каждой операции в хронологическом порядке, расходуя вычет
/// последовательно, как это делает ФНС. Результат — разбивка по месяцам.
pub fn compute(entries: &[EntryLite], rules: &TaxRules) -> Vec<MonthReport> {
    let mut ordered: Vec<&EntryLite> = entries
        .iter()
        .filter(|e| e.amount_kop > 0 && parse_ym(&e.date).is_some())
        .collect();
    ordered.sort_by(|a, b| a.date.cmp(&b.date));

    let mut buckets: BTreeMap<(i64, i64), (i64, i64, i64, i64, i64)> = BTreeMap::new();
    let mut left = rules.deduction_kop;
    for e in ordered {
        let Some((year, month)) = parse_ym(&e.date) else {
            continue;
        };
        let organization = e.payer_type == crate::models::PAYER_ORGANIZATION;
        let (tax, used) = tax_for_entry(e.amount_kop, organization, rules, left);
        left = (left - used).max(0);
        let slot = buckets.entry((year, month)).or_insert((0, 0, 0, 0, 0));
        if organization {
            slot.1 += e.amount_kop;
        } else {
            slot.0 += e.amount_kop;
        }
        slot.2 += 1;
        slot.3 += tax;
        slot.4 += used;
    }

    let mut out = Vec::with_capacity(buckets.len());
    for ((year, month), (ind, org, count, tax, used)) in buckets {
        out.push(MonthReport {
            year,
            month,
            income_individual_kop: ind,
            income_organization_kop: org,
            income_total_kop: ind + org,
            tax_kop: tax,
            deduction_used_kop: used,
            entries_count: count,
        });
    }
    out
}

pub fn deduction_left(rules: &TaxRules, months: &[MonthReport]) -> i64 {
    let used: i64 = months.iter().map(|m| m.deduction_used_kop).sum();
    (rules.deduction_kop - used).max(0)
}

pub fn sum_tax(months: &[MonthReport], year: i64, from_month: i64, to_month: i64) -> i64 {
    months
        .iter()
        .filter(|m| m.year == year && m.month >= from_month && m.month <= to_month)
        .map(|m| m.tax_kop)
        .sum()
}

pub fn sum_income(months: &[MonthReport], year: i64, from_month: i64, to_month: i64) -> (i64, i64) {
    months
        .iter()
        .filter(|m| m.year == year && m.month >= from_month && m.month <= to_month)
        .fold((0, 0), |acc, m| (acc.0 + m.income_total_kop, acc.1 + m.entries_count))
}

pub fn year_income(months: &[MonthReport], year: i64) -> i64 {
    months
        .iter()
        .filter(|m| m.year == year)
        .map(|m| m.income_total_kop)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> TaxRules {
        TaxRules {
            rate_individual: 400,
            rate_organization: 600,
            deduction_kop: 1_000_000,
        }
    }

    fn e(date: &str, kop: i64, payer: &str) -> EntryLite {
        EntryLite {
            date: date.into(),
            amount_kop: kop,
            payer_type: payer.into(),
        }
    }

    #[test]
    fn individual_only() {
        let m = compute(&[e("2026-01-15", 10_000_00, "individual")], &rules());
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].tax_kop, 400_00);
        assert_eq!(m[0].deduction_used_kop, 0);
    }

    #[test]
    fn organization_uses_deduction() {
        let m = compute(&[e("2026-01-15", 10_000_00, "organization")], &rules());
        assert_eq!(m[0].tax_kop, 400_00);
        assert_eq!(m[0].deduction_used_kop, 200_00);
    }

    #[test]
    fn deduction_exhausts_and_stops() {
        let m = compute(
            &[
                e("2026-01-10", 500_000_00, "organization"),
                e("2026-02-10", 10_000_00, "organization"),
            ],
            &rules(),
        );
        assert_eq!(m[0].deduction_used_kop, 10_000_00);
        assert_eq!(m[0].tax_kop, 2_000_000);
        assert_eq!(m[1].deduction_used_kop, 0);
        assert_eq!(m[1].tax_kop, 600_00);
    }

    #[test]
    fn partial_deduction_in_next_month() {
        let m = compute(
            &[
                e("2026-01-10", 50_000_00, "organization"),
                e("2026-02-10", 10_000_00, "organization"),
            ],
            &rules(),
        );
        assert_eq!(m[0].deduction_used_kop, 1_000_00);
        assert_eq!(m[0].tax_kop, 200_000);
        assert_eq!(m[1].deduction_used_kop, 200_00);
        assert_eq!(m[1].tax_kop, 400_00);
    }

    #[test]
    fn rounding_half_up() {
        let m = compute(&[e("2026-03-01", 33_33, "individual")], &rules());
        assert_eq!(m[0].tax_kop, 1_33);
    }

    #[test]
    fn mixed_payers_same_month() {
        let m = compute(
            &[
                e("2026-04-01", 1_000_00, "individual"),
                e("2026-04-02", 1_000_00, "organization"),
            ],
            &rules(),
        );
        // 4 % * 1000 + 6 % * 1000 = 100, вычет гасит 2 % от 1000 = 20
        assert_eq!(m[0].tax_kop, 80_00);
        assert_eq!(m[0].income_total_kop, 2_000_00);
    }

    #[test]
    fn months_are_ordered_and_split() {
        let m = compute(
            &[
                e("2026-02-01", 100_00, "individual"),
                e("2026-01-01", 100_00, "individual"),
            ],
            &rules(),
        );
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].year, m[0].month), (2026, 1));
        assert_eq!((m[1].year, m[1].month), (2026, 2));
    }

    #[test]
    fn invalid_and_zero_entries_ignored() {
        let m = compute(
            &[
                e("2026-13-01", 100_00, "individual"),
                e("bad", 100_00, "individual"),
                e("2026-05-01", 0, "individual"),
            ],
            &rules(),
        );
        assert!(m.is_empty());
    }

    #[test]
    fn deduction_remaining_is_capped() {
        let r = rules();
        let m = compute(&[e("2026-01-05", 1_000_000_00, "organization")], &r);
        assert_eq!(m[0].deduction_used_kop, r.deduction_kop);
        assert_eq!(deduction_left(&r, &m), 0);
    }
}
