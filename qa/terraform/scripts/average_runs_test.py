import unittest
import pandas as pd
import tempfile
import os
from average_runs import average_csv_files

class TestAverageCSV(unittest.TestCase):
    def create_temp_csv(self, rows):
        tmp = tempfile.NamedTemporaryFile(delete=False, mode='w', suffix=".csv")
        for row in rows:
            tmp.write(",".join(map(str, row)) + "\n")
        tmp.close()
        return tmp.name

    def test_average_of_three_files(self):
        rows1 = [["Node1", 0.0, 100], ["Node2", 0.0, 200]]
        rows2 = [["Node1", 0.0, 110], ["Node2", 0.0, 210]]
        rows3 = [["Node1", 0.0, 90],  ["Node2", 0.0, 190]]

        files = [self.create_temp_csv(rows) for rows in [rows1, rows2, rows3]]
        out_file = tempfile.NamedTemporaryFile(delete=False).name

        average_csv_files(files, out_file)

        result = pd.read_csv(out_file, header=None)
        expected = pd.DataFrame([["Node1", 0.0, 100], ["Node2", 0.0, 200]])
        pd.testing.assert_frame_equal(result, expected)

        # Cleanup
        for f in files + [out_file]:
            os.remove(f)

    def test_single_file_pass_through(self):
        rows = [["Node1", 0.0, 123], ["Node2", 0.0, 456]]
        input_file = self.create_temp_csv(rows)
        output_file = tempfile.NamedTemporaryFile(delete=False).name

        average_csv_files([input_file], output_file)
        result = pd.read_csv(output_file, header=None)
        expected = pd.DataFrame(rows)

        pd.testing.assert_frame_equal(result, expected)

        os.remove(input_file)
        os.remove(output_file)

if __name__ == '__main__':
    unittest.main()
