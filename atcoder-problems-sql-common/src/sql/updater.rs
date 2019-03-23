use diesel::PgConnection;
use diesel::{connection::SimpleConnection, QueryResult};

pub trait SqlUpdater {
    fn update_accepted_count(&self) -> QueryResult<()>;
    fn update_problem_solver_count(&self) -> QueryResult<()>;
    fn update_rated_point_sums(&self) -> QueryResult<()>;
    fn update_language_count(&self) -> QueryResult<()>;
    fn update_great_submissions(&self) -> QueryResult<()>;
    fn aggregate_great_submissions(&self) -> QueryResult<()>;
    fn update_problem_points(&self) -> QueryResult<()>;
}

impl SqlUpdater for PgConnection {
    fn update_accepted_count(&self) -> QueryResult<()> {
        self.batch_execute(
            r"
            DELETE FROM
                accepted_count;
            INSERT INTO
                accepted_count (user_id, problem_count)
            SELECT
                user_id,
                COUNT(DISTINCT(problem_id))
            FROM
                submissions
            WHERE
                result = 'AC'
            GROUP BY
                user_id;
            ",
        )
    }
    fn update_problem_solver_count(&self) -> QueryResult<()> {
        self.batch_execute(
            r"
            DELETE FROM
                solver;
            INSERT INTO
                solver (user_count, problem_id)
            SELECT
                COUNT(DISTINCT(user_id)),
                problem_id
            FROM
                submissions
            WHERE
                result = 'AC'
            GROUP BY
                problem_id;
            ",
        )
    }

    fn update_rated_point_sums(&self) -> QueryResult<()> {
        self.batch_execute(
            r"
            DELETE FROM
                rated_point_sum;
            INSERT INTO
                rated_point_sum (point_sum, user_id)
            SELECT
                SUM(point),
                user_id
            FROM
                (
                    SELECT
                        DISTINCT(submissions.user_id, submissions.problem_id),
                        points.point,
                        submissions.user_id
                    FROM
                        submissions
                        JOIN points ON points.problem_id = submissions.problem_id
                    WHERE
                        result = 'AC'
                        AND points.point IS NOT NULL
                        AND submissions.user_id NOT LIKE 'vjudge_'
                ) AS sub
            GROUP BY
                user_id;
        ",
        )
    }

    fn update_language_count(&self) -> QueryResult<()> {
        self.batch_execute(
            r"
                DELETE FROM
                    language_count;
                INSERT INTO
                    language_count (user_id, simplified_language, problem_count)
                SELECT
                    user_id,
                    simplified_language,
                    COUNT(DISTINCT(problem_id))
                FROM
                    (
                        SELECT
                            regexp_replace(language, '((?<!Perl)\d*|) \(.*\)', '') AS simplified_language,
                            user_id,
                            problem_id
                        FROM
                            submissions
                        WHERE
                            result = 'AC'
                    ) AS sub
                GROUP BY
                    (simplified_language, user_id);
                ",
        )
    }

    fn update_great_submissions(&self) -> QueryResult<()> {
        let query = [
            ("first", "epoch_second"),
            ("fastest", "execution_time"),
            ("shortest", "length"),
        ]
        .into_iter()
        .map(|(table, column)| {
            format!(
                r"
                DELETE FROM
                    {table};
                INSERT INTO
                    {table} (submission_id, problem_id, contest_id)
                SELECT
                    id,
                    problem_id,
                    contest_id
                FROM
                    (
                        SELECT
                            submissions.id,
                            submissions.problem_id,
                            submissions.contest_id,
                            ROW_NUMBER() OVER(
                                PARTITION BY problem_id
                                ORDER BY
                                    submissions.{column} ASC,
                                    submissions.id ASC
                            ) ordering
                        FROM
                            submissions
                            INNER JOIN contests ON submissions.contest_id = contests.id
                        WHERE
                            submissions.result = 'AC'
                            AND submissions.epoch_second > contests.start_epoch_second
                    ) AS a
                WHERE
                    ordering = 1;",
                table = table,
                column = column
            )
        })
        .fold(String::new(), |mut acc, q| {
            acc.push_str(&q);
            acc
        });
        self.batch_execute(&query)
    }

    fn aggregate_great_submissions(&self) -> QueryResult<()> {
        for (table, parent) in [
            ("first_submission_count", "first"),
            ("shortest_submission_count", "shortest"),
            ("fastest_submission_count", "fastest"),
        ]
        .into_iter()
        {
            self.batch_execute(&format!(
                r"
                DELETE FROM
                    {table};
                INSERT INTO
                    {table} (problem_count, user_id)
                SELECT
                    COUNT(DISTINCT({parent}.problem_id)),
                    user_id
                FROM
                    {parent}
                    JOIN submissions ON submissions.id = {parent}.submission_id
                GROUP BY
                    submissions.user_id;
                ",
                table = table,
                parent = parent
            ))?
        }
        Ok(())
    }

    fn update_problem_points(&self) -> QueryResult<()> {
        self.batch_execute(
            r"
                DELETE FROM
                    points
                WHERE
                    point IS NOT NULL;
                INSERT INTO
                    points (problem_id, point)
                SELECT
                    submissions.problem_id,
                    MAX(submissions.point)
                FROM
                    submissions
                    INNER JOIN contests ON contests.id = submissions.contest_id
                WHERE
                    contests.start_epoch_second >= 1468670400
                    AND contests.rate_change != '-'
                GROUP BY
                    submissions.problem_id;
            ",
        )
    }
}