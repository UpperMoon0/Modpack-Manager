import logging
import os
import tempfile
import zipfile

import requests

from model.url_registries import UrlRegistries


class JDKService:
    def __init__(self):
        self.logger = logging.getLogger(__name__)
        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s',
        )

    def download_jdk(self, url_registries: UrlRegistries, progress_callback=None):
        # Get the system's temp directory and the file path
        temp_dir = tempfile.gettempdir()
        file_path = os.path.join(temp_dir, 'jdk.rar')

        url = url_registries.jdk_url

        with requests.get(url, stream=True) as response:
            if response.status_code != 200:
                self.logger.error(f"Failed to download jdk: {response.status_code}")
                raise Exception(f"Failed to download jdk: {response.status_code}")

            total_size = int(response.headers.get('Content-Length', 0))
            bytes_downloaded = 0

            # Use the progress callback
            with open(file_path, 'wb') as file:
                for chunk in response.iter_content(chunk_size=1024):
                    if chunk:
                        file.write(chunk)
                        bytes_downloaded += len(chunk)

                        # Call the progress callback
                        if progress_callback:
                            progress_callback(bytes_downloaded, total_size)

        return file_path


    def install_jdk(self):
        jdk_path = 'C:/Program Files/Java'

        if not os.path.exists(jdk_path):
            os.makedirs(jdk_path)

        temp_jdk_path = os.path.join(tempfile.gettempdir(), 'jdk.rar')

        if not zipfile.is_zipfile(temp_jdk_path):
            self.logger.error(f"JDK file is not a zip file: {temp_jdk_path}")
            raise Exception(f"File {temp_jdk_path} is not a zip file")

        with zipfile.ZipFile(temp_jdk_path, 'r') as zip_ref:
            zip_ref.extractall(jdk_path)

        # Set the JAVA_HOME environment variable
        os.environ['JAVA_HOME'] = jdk_path

        # Add the JDK bin directory to the PATH environment variable
        jdk_bin_path = os.path.join(jdk_path, 'bin')
        os.environ['PATH'] = jdk_bin_path + os.pathsep + os.environ.get('PATH', '')

        # Delete the downloaded file
        os.remove(temp_jdk_path)
        self.logger.info("JDK installed successfully and environment variables set.")

